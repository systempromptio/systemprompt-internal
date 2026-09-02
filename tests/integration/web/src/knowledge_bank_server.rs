//! `KnowledgeBankServer` constructed against a real pool.
//!
//! Construction is the part worth a database: it builds a `ToolUsageRepository`
//! and an `McpArtifactRepository` off the pool and opens the document store
//! over it. The advertised surface (`get_info`, the tool list) is asserted
//! alongside it so a capability or tool-name change cannot land silently.

use std::sync::Arc;

use rmcp::ServerHandler;
use sqlx::PgPool;
use systemprompt::database::Database;
use systemprompt::identifiers::McpServerId;
use systemprompt::security::authz::{DenyAllHook, SharedAuthzHook};
use systemprompt_mcp_knowledge_bank::store::KnowledgeStore;
use systemprompt_mcp_knowledge_bank::{KnowledgeBankServer, tools};

use crate::tempdb::TempDb;

fn hook() -> SharedAuthzHook {
    Arc::new(DenyAllHook::null())
}

fn server(pool: &Arc<PgPool>) -> KnowledgeBankServer {
    let db_pool = Arc::new(Database::from_pools(
        Arc::clone(pool),
        Some(Arc::clone(pool)),
    ));
    KnowledgeBankServer::new(db_pool, McpServerId::new("knowledge-bank"), hook())
        .expect("construct the knowledge-bank server against a live pool")
}

#[tokio::test]
async fn new_builds_its_repositories_from_the_pool() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let built = server(&db.pool);

    assert!(
        built.get_info().instructions.is_some(),
        "a constructed server advertises its usage instructions"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn get_info_advertises_tools_and_names_the_service_id() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let built = server(&db.pool);

    let info = built.get_info();

    assert!(
        info.capabilities.tools.is_some(),
        "the knowledge bank is a tool server"
    );
    assert!(
        info.capabilities.resources.is_some(),
        "it serves the artifact-viewer ui:// resources"
    );
    assert_eq!(
        info.server_info.name, "Knowledge Bank (knowledge-bank)",
        "the service id the caller passed is carried into the server name"
    );
    assert_eq!(
        info.server_info.title.as_deref(),
        Some("Company Knowledge Bank")
    );
    assert!(
        !info.server_info.version.is_empty(),
        "the crate version is reported"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn get_info_is_stable_across_service_ids_apart_from_the_name() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let db_pool = Arc::new(Database::from_pools(
        Arc::clone(&db.pool),
        Some(Arc::clone(&db.pool)),
    ));
    let renamed = KnowledgeBankServer::new(db_pool, McpServerId::new("kb-staging"), hook())
        .expect("construct with a different service id");

    let info = renamed.get_info();

    assert_eq!(info.server_info.name, "Knowledge Bank (kb-staging)");
    assert_eq!(
        info.instructions,
        server(&db.pool).get_info().instructions,
        "the instructions do not depend on the service id"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn the_instructions_point_callers_at_search_before_advising() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let built = server(&db.pool);

    let instructions = built
        .get_info()
        .instructions
        .expect("the server ships instructions");

    assert!(
        instructions.contains(tools::TOOL_SEARCH),
        "the instructions name the search tool: {instructions}"
    );
    assert!(
        instructions.contains("admin-only"),
        "the instructions state that uploading is restricted: {instructions}"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn the_server_exposes_exactly_six_tools() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _built = server(&db.pool);

    let listed = tools::list_tools();
    let mut names: Vec<&str> = listed.iter().map(|t| t.name.as_ref()).collect();
    names.sort_unstable();

    let mut expected = vec![
        tools::TOOL_LIST,
        tools::TOOL_SEARCH,
        tools::TOOL_UPLOAD,
        tools::TOOL_PROPOSAL_LIST,
        tools::TOOL_PROPOSAL_GET,
        tools::TOOL_PROPOSAL_DECIDE,
    ];
    expected.sort_unstable();
    assert_eq!(
        names, expected,
        "the knowledge bank advertises search, list, upload and the three proposal tools — and \
         nothing else"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn every_advertised_tool_carries_a_description_and_an_object_schema() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _built = server(&db.pool);

    for tool in tools::list_tools() {
        assert!(
            tool.description
                .as_ref()
                .is_some_and(|d| !d.trim().is_empty()),
            "tool {} has no description",
            tool.name
        );
        assert_eq!(
            tool.input_schema.get("type").and_then(|t| t.as_str()),
            Some("object"),
            "tool {} must declare an object input schema",
            tool.name
        );
    }

    db.cleanup().await;
}

#[tokio::test]
async fn the_store_the_server_opens_starts_empty() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let _built = server(&db.pool);

    let store = KnowledgeStore::new(Arc::new(Database::from_pools(
        Arc::clone(&db.pool),
        Some(Arc::clone(&db.pool)),
    )));

    // Deliberate: there are no fixtures. A knowledge bank that answers with
    // invented context is worse than one that answers with nothing.
    assert_eq!(store.count().await.expect("count the new bank"), 0);
    assert!(
        store
            .list_documents(None, None)
            .await
            .expect("list the new bank")
            .is_empty()
    );

    db.cleanup().await;
}

#[tokio::test]
async fn the_registered_schema_reaches_the_database() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    // TempDb installs schemas from the extension registrations linked into
    // this binary. If the knowledge bank's `register_extension!` stopped being
    // picked up, every query below would fail on a missing relation rather
    // than returning zero rows.
    let indexed: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM pg_indexes WHERE tablename = 'knowledge_documents' \
         AND indexdef ILIKE '%gin%')",
    )
    .fetch_one(db.pool.as_ref())
    .await
    .expect("query pg_indexes");

    assert!(
        indexed,
        "the GIN index on content_tsv is what makes ranked search a search rather than a scan"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn constructing_two_servers_on_one_pool_is_independent() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let first = server(&db.pool);
    let second = server(&db.pool);

    assert_eq!(
        first.get_info().server_info.name,
        second.get_info().server_info.name,
        "two servers built from the same pool advertise the same identity"
    );

    db.cleanup().await;
}

// `initialize`, `list_tools`, and `call_tool` take an rmcp
// `RequestContext<RoleServer>`, which owns a `Peer` that only exists once a
// transport is running — there is no constructor for one in a test process.
// The tool list is therefore asserted through `tools::list_tools`, the same
// value `list_tools` returns, and `dispatch_tool` (including its unknown-tool
// branch) is `pub(super)` and unreachable from outside the crate.
