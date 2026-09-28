//! `repositories::analytics::{conversation_rows, agents, tools}` — the
//! conversations index and its totals, and the per-agent / per-tool rollups.
//!
//! The fixture inserts requests with no `ai_request_payloads` row, so a lone
//! request in a context is a side call by the view's rule; the tests that
//! want one to be listed ask for side calls explicitly.

use systemprompt::identifiers::{ContextId, UserId};
use systemprompt_web_admin::repositories::analytics::conversation_rows::{
    ConversationFilter, ConversationPage, get_conversation_totals, list_conversations_paged,
    list_distinct_models,
};
use systemprompt_web_admin::repositories::analytics::{list_agents, list_tools};

use crate::fixtures::{
    EventSpec, RequestSpec, insert_event, insert_request, insert_user, new_context_id,
    project_scope, set_project, unclaimed_email, unique,
};
use crate::tempdb::TempDb;

#[tokio::test]
async fn list_conversations_reports_a_contexts_rollup() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("ctx")).await;
    let context = new_context_id();
    for _ in 0..2 {
        let mut spec = RequestSpec::completed(&unique("req"), &user);
        spec.context_id = Some(&context);
        insert_request(&db.pool, &spec).await;
    }
    let filter = ConversationFilter {
        user_id: Some(user.clone()),
        ..ConversationFilter::default()
    };

    let rows = list_conversations_paged(&db.pool, &filter, first_page())
        .await
        .expect("query succeeds")
        .0;

    let row = rows
        .iter()
        .find(|r| r.context_id.as_str() == context)
        .expect("the context appears");
    assert_eq!(row.turn_count, 2);
    assert_eq!(row.side_call_count, 0);
    assert_eq!(row.total_input_tokens, 200);
    assert_eq!(row.total_cost_microdollars, 10_000);
    assert_eq!(row.error_count, 0);
    assert_eq!(
        row.tool_call_count, 0,
        "no ai_request_tool_calls rows means no tool calls to count"
    );
    assert!(
        row.title.starts_with("Conversation "),
        "no name, no prompt: the id stands in"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn list_conversations_filters_to_one_user() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let mine = insert_user(&db.pool, &unique("user"), &unclaimed_email("ctxmine")).await;
    let theirs = insert_user(&db.pool, &unique("user"), &unclaimed_email("ctxtheirs")).await;
    for owner in [&mine, &theirs] {
        let context = new_context_id();
        let mut spec = RequestSpec::completed(&unique("req"), owner);
        spec.context_id = Some(&context);
        insert_request(&db.pool, &spec).await;
    }
    let filter = ConversationFilter {
        user_id: Some(mine.clone()),
        include_side_calls: true,
        ..ConversationFilter::default()
    };

    let rows = list_conversations_paged(&db.pool, &filter, first_page())
        .await
        .expect("query succeeds")
        .0;

    assert!(
        rows.iter()
            .all(|r| r.user_id.as_ref().is_none_or(|u| *u == mine)),
        "the filter must not leak another user's contexts"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn list_conversations_counts_failed_requests_as_errors() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("ctxerr")).await;
    let context = new_context_id();
    let mut failed = RequestSpec::completed(&unique("req"), &user);
    failed.context_id = Some(&context);
    failed.status = "failed";
    insert_request(&db.pool, &failed).await;
    let filter = ConversationFilter {
        user_id: Some(user.clone()),
        include_side_calls: true,
        error_only: true,
        ..ConversationFilter::default()
    };

    let rows = list_conversations_paged(&db.pool, &filter, first_page())
        .await
        .expect("query succeeds")
        .0;

    let row = rows
        .iter()
        .find(|r| r.context_id.as_str() == context)
        .expect("the context appears");
    assert_eq!(row.error_count, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn get_conversation_totals_sums_only_the_filtered_user() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("kpi")).await;
    let noise = insert_user(&db.pool, &unique("user"), &unclaimed_email("noise")).await;
    let context = new_context_id();
    let mut mine = RequestSpec::completed(&unique("req"), &user);
    mine.context_id = Some(&context);
    insert_request(&db.pool, &mine).await;
    // Why: only a harness-bound context (keyed on a client session) classifies a
    // lone tool-less request as a utility side call; elsewhere it is a turn.
    sqlx::query("UPDATE ai_requests SET client_session_id = $1 WHERE id = $2")
        .bind(unique("csid"))
        .bind(&mine.id)
        .execute(db.pool.as_ref())
        .await
        .expect("bind the request to a client session");
    let other = new_context_id();
    let mut theirs = RequestSpec::completed(&unique("req"), &noise);
    theirs.context_id = Some(&other);
    insert_request(&db.pool, &theirs).await;
    let filter = ConversationFilter {
        user_id: Some(user.clone()),
        include_side_calls: true,
        ..ConversationFilter::default()
    };

    let totals = get_conversation_totals(&db.pool, &filter)
        .await
        .expect("query succeeds");

    assert_eq!(totals.conversations, 1);
    assert_eq!(totals.users, 1);
    assert_eq!(totals.turns + totals.side_calls, 1);
    assert_eq!(
        totals.side_calls, 1,
        "a lone tool-less request in a harness-bound context is a side call"
    );
    assert_eq!(totals.total_cost_microdollars, 5_000);
    assert_eq!(totals.side_call_cost_microdollars, 5_000);
    db.cleanup().await;
}

#[tokio::test]
async fn get_conversation_totals_returns_a_row_even_when_nothing_matches() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let filter = ConversationFilter {
        user_id: Some(UserId::new(unique("absent"))),
        ..ConversationFilter::default()
    };

    let totals = get_conversation_totals(&db.pool, &filter)
        .await
        .expect("get_ over an aggregate always has a row to return");

    assert_eq!(totals.conversations, 0);
    assert_eq!(totals.total_cost_microdollars, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn list_distinct_models_only_reports_models_used_inside_a_context() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("models")).await;
    let context = new_context_id();
    let mut inside = RequestSpec::completed(&unique("req"), &user);
    inside.context_id = Some(&context);
    inside.model = "context-model";
    insert_request(&db.pool, &inside).await;
    let mut outside = RequestSpec::completed(&unique("req"), &user);
    outside.model = "contextless-model";
    insert_request(&db.pool, &outside).await;

    let models = list_distinct_models(&db.pool)
        .await
        .expect("query succeeds");

    assert!(models.contains(&"context-model".to_owned()));
    assert!(!models.contains(&"contextless-model".to_owned()));
    db.cleanup().await;
}

#[tokio::test]
async fn list_tools_rolls_up_recent_tool_events() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("tools")).await;
    let session = unique("session");
    let tool = unique("tool");
    for _ in 0..2 {
        let event_id = unique("evt");
        let mut spec = EventSpec::tool_use(&event_id, &user, &session);
        spec.tool_name = Some(&tool);
        insert_event(&db.pool, &spec).await;
    }
    let error_id = unique("evt");
    let mut failure = EventSpec::tool_use(&error_id, &user, &session);
    failure.tool_name = Some(&tool);
    failure.event_type = "claude_code_ToolFailure";
    insert_event(&db.pool, &failure).await;

    let rows = list_tools(&db.pool).await.expect("query succeeds");

    let row = rows
        .iter()
        .find(|r| r.tool_name == tool)
        .expect("the tool appears");
    assert_eq!(row.calls, 3);
    assert_eq!(row.errors, 1);
    assert_eq!(row.sessions, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn list_agents_keys_on_the_metadata_agent_id_before_the_plugin() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("agents")).await;
    let session = unique("session");
    let event_id = unique("evt");
    insert_event(&db.pool, &EventSpec::tool_use(&event_id, &user, &session)).await;
    let agent = unique("agent");
    sqlx::query(
        "UPDATE plugin_usage_events SET metadata = $2, plugin_id = 'some-plugin' WHERE id = $1",
    )
    .bind(&event_id)
    .bind(serde_json::json!({ "agent_id": agent }))
    .execute(&*db.pool)
    .await
    .expect("attach the agent id");

    let rows = list_agents(&db.pool).await.expect("query succeeds");

    let row = rows
        .iter()
        .find(|r| r.agent_id.as_str() == agent)
        .expect("the agent id wins over the plugin id");
    assert_eq!(row.calls, 1);
    assert_eq!(row.sessions, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn context_ids_round_trip_through_the_typed_identifier() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("typed")).await;
    // `ContextId::new` validates UUID v4, so the round trip only holds for the
    // shape production writes — the other tests here store opaque strings that
    // survive the read path but would not survive re-validation.
    let context = ContextId::generate().to_string();
    let mut spec = RequestSpec::completed(&unique("req"), &user);
    spec.context_id = Some(&context);
    insert_request(&db.pool, &spec).await;
    let filter = ConversationFilter {
        user_id: Some(user.clone()),
        include_side_calls: true,
        ..ConversationFilter::default()
    };

    let rows = list_conversations_paged(&db.pool, &filter, first_page())
        .await
        .expect("query succeeds")
        .0;

    assert_eq!(rows.len(), 1);
    assert_eq!(
        rows[0].context_id,
        ContextId::try_new(context).expect("valid fixture identifier")
    );
    db.cleanup().await;
}

#[tokio::test]
async fn list_conversations_filters_by_project() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let commerce = insert_user(&db.pool, &unique("user"), &unclaimed_email("ctxcommerce")).await;
    let core = insert_user(&db.pool, &unique("user"), &unclaimed_email("ctxcore")).await;
    set_project(&db.pool, &commerce, Some("commerce")).await;
    set_project(&db.pool, &core, Some("core")).await;
    for owner in [&commerce, &core] {
        let context = new_context_id();
        let mut spec = RequestSpec::completed(&unique("req"), owner);
        spec.context_id = Some(&context);
        insert_request(&db.pool, &spec).await;
    }
    let scope = project_scope(&db.pool, "commerce").await;
    let filter = ConversationFilter {
        subject_ids: scope.as_sql().map(<[String]>::to_vec),
        include_side_calls: true,
        ..ConversationFilter::default()
    };

    let rows = list_conversations_paged(&db.pool, &filter, first_page())
        .await
        .expect("query succeeds")
        .0;

    assert!(
        rows.iter().any(|r| r.user_id.as_ref() == Some(&commerce)),
        "the commerce context is listed"
    );
    assert!(
        rows.iter().all(|r| r.user_id.as_ref() != Some(&core)),
        "the filter must not leak another project's contexts"
    );
    db.cleanup().await;
}

fn first_page() -> ConversationPage {
    ConversationPage {
        limit: 50,
        ..ConversationPage::default()
    }
}
