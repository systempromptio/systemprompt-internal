//! The knowledge bank's behaviour against a real `knowledge_documents` table.
//!
//! `mcp_dispatch` pins the happy path of each tool and the admin refusal; this
//! module drives what only Postgres can answer — that ranking really is
//! `ts_rank_cd` and not insertion order, that the two search fallbacks fire
//! when the tokenizer or the query gives the ranked path nothing to work with,
//! that the filters compose, that the size cap refuses before anything is
//! written, and that a document's attribution comes from the authenticated
//! caller rather than the payload.

use std::sync::Arc;

use rmcp::model::CallToolRequestParams;
use sqlx::PgPool;
use systemprompt::database::Database;
use systemprompt::identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt::mcp::repository::ToolUsageRepository;
use systemprompt::mcp::{McpArtifactRepository, McpToolExecutor};
use systemprompt::models::auth::{AuthenticatedUser, Permission};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt_mcp_knowledge_bank::server::tool::dispatch_tool;
use systemprompt_mcp_knowledge_bank::store::{KnowledgeStore, MAX_CONTENT_BYTES, NewDocument};
use systemprompt_mcp_knowledge_bank::tools::{TOOL_LIST, TOOL_SEARCH, TOOL_UPLOAD};

use crate::tempdb::TempDb;

const PROJECT: &str = "acme-storefront";

fn executor(pool: &Arc<PgPool>) -> McpToolExecutor {
    let usage = Arc::new(ToolUsageRepository::new(&db_pool(pool)).expect("tool usage repository"));
    let artifacts =
        Arc::new(McpArtifactRepository::new(&db_pool(pool)).expect("artifact repository"));
    McpToolExecutor::new(usage, artifacts, "knowledge-bank")
}

fn db_pool(pool: &Arc<PgPool>) -> systemprompt::database::DbPool {
    Arc::new(Database::from_pools(
        Arc::clone(pool),
        Some(Arc::clone(pool)),
    ))
}

fn store(pool: &Arc<PgPool>) -> KnowledgeStore {
    KnowledgeStore::new(db_pool(pool))
}

fn request_context() -> SysRequestContext {
    SysRequestContext::new(
        SessionId::new("kb-edge-session"),
        TraceId::new("kb-edge-trace"),
        ContextId::new_unchecked("00000000-0000-4000-8000-00000000e46e"),
        AgentName::new("kb-edge-agent"),
    )
}

fn admin_context() -> SysRequestContext {
    request_context().with_user(AuthenticatedUser::new(
        uuid::Uuid::new_v4(),
        "kb-admin".to_owned(),
        "kb-admin@example.com".to_owned(),
        vec![Permission::Admin],
    ))
}

fn call(tool: &'static str, arguments: serde_json::Value) -> CallToolRequestParams {
    let object = arguments
        .as_object()
        .expect("tool arguments are a JSON object")
        .clone();
    CallToolRequestParams::new(tool).with_arguments(object)
}

fn body_of(result: &rmcp::model::CallToolResult) -> String {
    result
        .structured_content
        .as_ref()
        .and_then(|v| v.pointer("/content"))
        .and_then(|v| v.as_str())
        .expect("the executor returns the handler's artifact as structured content")
        .to_owned()
}

fn summary_of(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

async fn dispatch(
    db: &TempDb,
    ctx: &SysRequestContext,
    tool: &'static str,
    arguments: serde_json::Value,
) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
    let executor = executor(&db.pool);
    let request = call(tool, arguments);
    let profile = client();
    dispatch_tool(
        &systemprompt_mcp_knowledge_bank::server::tool::Dispatch {
            executor: &executor,
            request: &request,
            request_context: ctx,
            client: &profile,
        },
        &store(&db.pool),
        tool,
    )
    .await
}

fn client() -> systemprompt::mcp::ClientProfile {
    systemprompt::mcp::ClientProfile {
        protocol_version: Some(rmcp::model::ProtocolVersion::V_2025_06_18),
        ..systemprompt::mcp::ClientProfile::default()
    }
}


// Three documents whose word counts make the ranking predictable.
async fn seed(db: &TempDb) {
    let store = store(&db.pool);
    for (title, source, project, content) in [
        (
            "Checkout workshop",
            "meeting-transcript",
            Some(PROJECT),
            "Guest checkout was agreed. The checkout flow keeps checkout as one page.",
        ),
        (
            "Payment provider decision",
            "document",
            Some(PROJECT),
            "We chose the incumbent provider. Checkout is unaffected.",
        ),
        (
            "Unfiled note",
            "email",
            None,
            "A stray thought about warehousing.",
        ),
    ] {
        store
            .insert(NewDocument {
                title,
                source,
                project,
                content,
                uploaded_by: "seed-user",
            })
            .await
            .expect("seed insert");
    }
}

#[tokio::test]
async fn a_new_knowledge_bank_starts_empty() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let result = dispatch(&db, &request_context(), TOOL_LIST, serde_json::json!({}))
        .await
        .expect("listing an empty bank is a successful call");

    assert!(
        body_of(&result).contains("holds no documents"),
        "a fresh bank has nothing in it — no fixtures, no seeded context: {}",
        body_of(&result)
    );

    db.cleanup().await;
}

#[tokio::test]
async fn search_ranks_by_term_density_rather_than_insertion_order() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    let result = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "checkout" }),
    )
    .await
    .expect("search dispatches to its handler");

    let body = body_of(&result);
    let workshop = body
        .find("Checkout workshop")
        .expect("the workshop matched");
    let decision = body
        .find("Payment provider decision")
        .expect("the decision matched too");
    assert!(
        workshop < decision,
        "the document that says checkout four times outranks the one that says it once: {body}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn search_stems_the_query_rather_than_matching_literally() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    // "agreed" is stored; "agree" is what a caller types. The English
    // configuration stems both to the same lexeme — this is the whole reason
    // for using a tsvector rather than LIKE.
    let result = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "agree" }),
    )
    .await
    .expect("search dispatches to its handler");

    assert!(
        body_of(&result).contains("Checkout workshop"),
        "the stemmer matched the stored form: {}",
        body_of(&result)
    );

    db.cleanup().await;
}

#[tokio::test]
async fn an_empty_query_lists_the_newest_documents_instead_of_nothing() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    let result = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "   " }),
    )
    .await
    .expect("an empty query is a successful call");

    let body = body_of(&result);
    assert!(
        !body.contains("No matching documents"),
        "an empty query orients the caller rather than refusing: {body}"
    );
    assert!(
        body.contains("Unfiled note"),
        "the newest document leads the fallback listing: {body}"
    );
    assert!(
        summary_of(&result).contains("most recent"),
        "the summary tells the model this was a fallback, not a match: {}",
        summary_of(&result)
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_partial_word_falls_back_to_substring_matching() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    // "warehous" produces a lexeme that matches nothing as a whole word, so
    // the ranked path returns zero rows and the ILIKE fallback takes over.
    let result = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "warehous" }),
    )
    .await
    .expect("search dispatches to its handler");

    assert!(
        body_of(&result).contains("Unfiled note"),
        "a prefix the tokenizer cannot use still finds the document: {}",
        body_of(&result)
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_genuine_miss_still_reports_the_sentinel() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    let result = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "zeppelin quantum walrus" }),
    )
    .await
    .expect("a miss is a successful call");

    assert!(
        body_of(&result).contains("No matching documents"),
        "neither fallback invents a match: {}",
        body_of(&result)
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_search_limit_caps_the_documents_returned_and_clamps_out_of_range_values() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    for limit in [1, 0] {
        let result = dispatch(
            &db,
            &request_context(),
            TOOL_SEARCH,
            serde_json::json!({ "query": "checkout", "limit": limit }),
        )
        .await
        .expect("search dispatches to its handler");

        assert_eq!(
            summary_of(&result).split(' ').next(),
            Some("1"),
            "a limit of {limit} yields one document: {}",
            summary_of(&result)
        );
    }

    let oversized = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "checkout", "limit": 9999 }),
    )
    .await
    .expect("an oversized limit is clamped, not rejected");
    let matched: usize = summary_of(&oversized)
        .split(' ')
        .next()
        .and_then(|n| n.parse().ok())
        .expect("the summary opens with the match count");
    assert_eq!(matched, 2, "the clamp cannot invent documents");

    db.cleanup().await;
}

#[tokio::test]
async fn the_project_filter_scopes_search_and_excludes_untagged_documents() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    let foreign = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "checkout", "project": "some-other-client" }),
    )
    .await
    .expect("a project-scoped miss is still a successful call");
    assert!(body_of(&foreign).contains("No matching documents"));

    let scoped = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "checkout", "project": PROJECT }),
    )
    .await
    .expect("search dispatches to its handler");
    assert!(body_of(&scoped).contains("Checkout workshop"));

    db.cleanup().await;
}

#[tokio::test]
async fn listing_applies_the_project_and_source_filters_together() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    let by_source = dispatch(
        &db,
        &request_context(),
        TOOL_LIST,
        serde_json::json!({ "source": "document" }),
    )
    .await
    .expect("list dispatches to its handler");
    let body = body_of(&by_source);
    assert_eq!(body.lines().count(), 1);
    assert!(body.contains("Payment provider decision"), "{body}");

    let contradictory = dispatch(
        &db,
        &request_context(),
        TOOL_LIST,
        serde_json::json!({ "project": PROJECT, "source": "no-such-source" }),
    )
    .await
    .expect("list dispatches to its handler");
    assert!(
        body_of(&contradictory).contains("holds no documents matching the filter"),
        "a real project with a non-matching source filters everything out"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_listing_reports_sizes_and_never_the_content() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;

    let result = dispatch(&db, &request_context(), TOOL_LIST, serde_json::json!({}))
        .await
        .expect("list dispatches to its handler");

    let body = body_of(&result);
    assert_eq!(body.lines().count(), 3);
    assert!(body.contains("chars"), "sizes are reported: {body}");
    assert!(
        !body.contains("Guest checkout was agreed"),
        "listing is a browse affordance, not a content dump: {body}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn an_uploaded_document_is_searchable_immediately_and_says_how_to_find_it() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let uploaded = dispatch(
        &db,
        &admin_context(),
        TOOL_UPLOAD,
        serde_json::json!({
            "title": "Zanzibar Migration Sync",
            "source": "meeting-transcript",
            "project": PROJECT,
            "content": "We agreed to defer the zanzibar migration.",
        }),
    )
    .await
    .expect("an admin may upload");

    let receipt = body_of(&uploaded);
    assert!(
        receipt.contains("search_project_context"),
        "the receipt names the tool that retrieves the document again: {receipt}"
    );
    assert!(
        receipt.contains("Zanzibar Migration Sync") && receipt.contains(PROJECT),
        "the receipt carries the arguments that retrieval needs: {receipt}"
    );

    let found = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "zanzibar" }),
    )
    .await
    .expect("search dispatches to its handler");
    assert!(
        body_of(&found).contains("Zanzibar Migration Sync"),
        "the generated tsvector made the row searchable on write"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_document_is_attributed_to_the_authenticated_caller() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let admin = admin_context();
    let expected = admin.user_id().to_string();

    dispatch(
        &db,
        &admin,
        TOOL_UPLOAD,
        serde_json::json!({
            "title": "Attribution check",
            "source": "document",
            // A payload field the server must ignore — provenance that a
            // client can name is provenance a client can forge.
            "uploaded_by": "somebody-else",
            "content": "Recorded against the caller, not the payload.",
        }),
    )
    .await
    .expect("an admin may upload");

    let found = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "attribution" }),
    )
    .await
    .expect("search dispatches to its handler");

    let body = body_of(&found);
    assert!(
        body.contains(&format!("uploaded by: {expected}")),
        "the row is attributed to the authenticated user: {body}"
    );
    assert!(
        !body.contains("somebody-else"),
        "a payload-supplied uploader is ignored: {body}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn an_oversized_document_is_refused_before_anything_is_written() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let error = dispatch(
        &db,
        &admin_context(),
        TOOL_UPLOAD,
        serde_json::json!({
            "title": "Runaway paste",
            "source": "document",
            "content": "x".repeat(MAX_CONTENT_BYTES + 1),
        }),
    )
    .await
    .expect_err("a document over the cap is refused");

    assert!(
        error.message.contains("at most"),
        "the refusal states the ceiling: {}",
        error.message
    );

    let listed = dispatch(&db, &request_context(), TOOL_LIST, serde_json::json!({}))
        .await
        .expect("list dispatches to its handler");
    assert!(
        body_of(&listed).contains("holds no documents"),
        "the refused upload left no row behind"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_blank_required_field_is_refused_by_name() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let error = dispatch(
        &db,
        &admin_context(),
        TOOL_UPLOAD,
        serde_json::json!({
            "title": "   ",
            "source": "document",
            "content": "Body present, title is not.",
        }),
    )
    .await
    .expect_err("a blank title is refused");

    assert!(
        error.message.contains("title"),
        "the refusal names the offending field: {}",
        error.message
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_blank_project_is_stored_as_unscoped_rather_than_as_an_empty_tag() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    dispatch(
        &db,
        &admin_context(),
        TOOL_UPLOAD,
        serde_json::json!({
            "title": "Untagged",
            "source": "email",
            "project": "   ",
            "content": "No project was meant here.",
        }),
    )
    .await
    .expect("an admin may upload");

    let listed = dispatch(&db, &request_context(), TOOL_LIST, serde_json::json!({}))
        .await
        .expect("list dispatches to its handler");

    assert!(
        body_of(&listed).contains("unscoped"),
        "an all-whitespace project is NULL, not a tag no filter will ever match: {}",
        body_of(&listed)
    );

    db.cleanup().await;
}

// Why: the bank holds two kinds of row in one table — documents an admin
// curated (`status = 'reference'`, what `insert` writes) and inbound business
// email the brain@ pipeline captured. The role grant on this server is
// user-wide, so the status filter is the only thing standing between a user
// and someone's mail. These three tests are that filter.
async fn seed_captured_mail(db: &TempDb) {
    sqlx::query(
        "INSERT INTO knowledge_documents (title, source, project, content, uploaded_by, status) \
         VALUES ($1, 'email', $2, $3, 'brain@systemprompt.io', 'proposed')",
    )
    .bind("Northwind rollout - call recap")
    .bind(PROJECT)
    .bind("Checkout is slipping a week. Their CFO wants the revised quote by Friday.")
    .execute(db.pool.as_ref())
    .await
    .expect("seed a captured email");
}

#[tokio::test]
async fn a_user_search_returns_curated_documents_and_never_captured_mail() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;
    seed_captured_mail(&db).await;

    let result = dispatch(
        &db,
        &request_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "checkout" }),
    )
    .await
    .expect("a non-admin may search the bank");
    let body = body_of(&result);

    assert!(
        body.contains("Checkout workshop"),
        "a curated document is what the read grant is for: {body}"
    );
    assert!(
        !body.contains("Northwind"),
        "captured mail matches this query but must never reach a non-admin: {body}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn an_admin_search_returns_the_whole_bank() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed(&db).await;
    seed_captured_mail(&db).await;

    let result = dispatch(
        &db,
        &admin_context(),
        TOOL_SEARCH,
        serde_json::json!({ "query": "checkout" }),
    )
    .await
    .expect("an admin may search the bank");
    let body = body_of(&result);

    assert!(
        body.contains("Northwind"),
        "an admin reads the pipeline rows too — that is what the two knowledge dashboards are \
         built on: {body}"
    );
    assert!(
        body.contains("Checkout workshop"),
        "and the curated documents alongside them: {body}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_user_listing_hides_captured_mail_and_upload_marks_a_document_reference() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    seed_captured_mail(&db).await;

    let uploaded = dispatch(
        &db,
        &admin_context(),
        TOOL_UPLOAD,
        serde_json::json!({
            "title": "Five layers",
            "source": "whitepaper",
            "project": "positioning",
            "content": "The workforce layer has no neutral incumbent.",
        }),
    )
    .await
    .expect("an admin may upload");
    assert!(!summary_of(&uploaded).is_empty());

    let status: String =
        sqlx::query_scalar("SELECT status FROM knowledge_documents WHERE source = 'whitepaper'")
            .fetch_one(db.pool.as_ref())
            .await
            .expect("read the uploaded document's status");
    assert_eq!(
        status, "reference",
        "upload_document lands outside the brain@ pipeline: the categorization job claims 'raw', \
         and 'reference' is what a non-admin may read"
    );

    let listed = dispatch(&db, &request_context(), TOOL_LIST, serde_json::json!({}))
        .await
        .expect("a non-admin may list the bank");
    let body = body_of(&listed);
    assert!(
        body.contains("Five layers"),
        "the curated document is listed: {body}"
    );
    assert!(
        !body.contains("Northwind"),
        "the captured email is not: {body}"
    );

    db.cleanup().await;
}
