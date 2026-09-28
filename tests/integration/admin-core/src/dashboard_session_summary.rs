//! Session summaries are projected from the accepted events: the refresh
//! drains the ingestion outbox the event's insert enqueued.
use systemprompt::identifiers::{SessionId, UserId};
use systemprompt_web_admin::repositories::dashboard::usage_aggregations;
use systemprompt_web_admin::repositories::marketplace::webhook;
use systemprompt_web_admin::types::{EVENT_POST_TOOL_USE, EVENT_POST_TOOL_USE_FAILURE};

use crate::fixtures::{insert_user, unclaimed_email, unique};
use crate::tempdb::TempDb;

fn summary_params<'a>(
    pool: &'a sqlx::PgPool,
    session_id: &'a SessionId,
    user_id: &'a UserId,
    event_type: &'a str,
) -> SessionSummaryParams<'a> {
    SessionSummaryParams {
        pool,
        session_id,
        user_id,
        event_type,
        content_input_bytes: 10,
        content_output_bytes: 5,
        is_subagent_stop: false,
        file_path: None,
        is_from_subagent: false,
    }
}

struct SummaryRow {
    total_events: i64,
    tool_uses: i64,
    prompts: i64,
    errors: i64,
    subagent_spawns: i64,
    user_prompts: Option<i32>,
    automated_actions: Option<i32>,
    unique_files_touched: Option<i32>,
}

// The column order of the SELECT below, named so the tuple stays readable.
type SummaryColumns = (
    i64,
    i64,
    i64,
    i64,
    i64,
    Option<i32>,
    Option<i32>,
    Option<i32>,
);

async fn read_summary(pool: &sqlx::PgPool, session_id: &str) -> SummaryRow {
    let row: SummaryColumns = sqlx::query_as(
        "SELECT total_events, tool_uses, prompts, errors, subagent_spawns,
                user_prompts, automated_actions, unique_files_touched
         FROM plugin_session_summaries WHERE session_id = $1",
    )
    .bind(session_id)
    .fetch_one(pool)
    .await
    .expect("read session summary");
    SummaryRow {
        total_events: row.0,
        tool_uses: row.1,
        prompts: row.2,
        errors: row.3,
        subagent_spawns: row.4,
        user_prompts: row.5,
        automated_actions: row.6,
        unique_files_touched: row.7,
    }
}

#[tokio::test]
async fn accept_and_refresh_creates_the_row_on_the_first_event() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summ")).await;
    let session = SessionId::new(unique("session"));

    accept_and_refresh(&summary_params(
        &db.pool,
        &session,
        &user,
        EVENT_POST_TOOL_USE,
    ))
    .await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(row.total_events, 1);
    assert_eq!(row.tool_uses, 1);
    assert_eq!(row.prompts, 0);
    assert_eq!(row.errors, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_accumulates_onto_the_same_session() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summacc")).await;
    let session = SessionId::new(unique("session"));
    let params = summary_params(&db.pool, &session, &user, EVENT_POST_TOOL_USE);

    for _ in 0..4 {
        accept_and_refresh(&params).await;
    }

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(row.total_events, 4);
    assert_eq!(row.tool_uses, 4);
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_classifies_a_failure_as_both_a_tool_use_and_an_error() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summfail")).await;
    let session = SessionId::new(unique("session"));

    accept_and_refresh(&summary_params(
        &db.pool,
        &session,
        &user,
        EVENT_POST_TOOL_USE_FAILURE,
    ))
    .await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(row.tool_uses, 1);
    assert_eq!(row.errors, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_counts_a_human_prompt_in_both_columns() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summprompt")).await;
    let session = SessionId::new(unique("session"));

    accept_and_refresh(&summary_params(
        &db.pool,
        &session,
        &user,
        "UserPromptSubmit",
    ))
    .await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(row.prompts, 1);
    assert_eq!(row.user_prompts, Some(1));
    assert_eq!(row.tool_uses, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_excludes_a_subagent_prompt_from_the_human_count() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summsub")).await;
    let session = SessionId::new(unique("session"));
    let mut params = summary_params(&db.pool, &session, &user, "UserPromptSubmit");
    params.is_from_subagent = true;

    accept_and_refresh(&params).await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(row.prompts, 1);
    assert_eq!(row.user_prompts, Some(0));
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_counts_a_subagent_tool_use_as_an_automated_action() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summauto")).await;
    let session = SessionId::new(unique("session"));
    let mut params = summary_params(&db.pool, &session, &user, EVENT_POST_TOOL_USE);
    params.is_from_subagent = true;

    accept_and_refresh(&params).await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(row.automated_actions, Some(1));
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_records_a_subagent_spawn() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summspawn")).await;
    let session = SessionId::new(unique("session"));
    let mut params = summary_params(&db.pool, &session, &user, "SubagentStop");
    params.is_subagent_stop = true;

    accept_and_refresh(&params).await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(row.subagent_spawns, 1);
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_recounts_the_files_touched_from_the_event_log() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summfiles")).await;
    let session = SessionId::new(unique("session"));
    for path in ["/a.rs", "/b.rs", "/a.rs"] {
        sqlx::query(
            "INSERT INTO plugin_usage_events (id, user_id, session_id, event_type, metadata)
             VALUES ($1, $2, $3, 'claude_code_PostToolUse',
                     jsonb_build_object('tool_input', jsonb_build_object('file_path', $4::text)))",
        )
        .bind(unique("event"))
        .bind(user.as_str())
        .bind(session.as_str())
        .bind(path)
        .execute(&*db.pool)
        .await
        .expect("insert event with a file path");
    }
    let mut params = summary_params(&db.pool, &session, &user, EVENT_POST_TOOL_USE);
    params.file_path = Some("/a.rs");

    accept_and_refresh(&params).await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert_eq!(
        row.unique_files_touched,
        Some(2),
        "distinct paths, not events"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn accept_and_refresh_leaves_the_file_count_untouched_without_a_path() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("summnofile")).await;
    let session = SessionId::new(unique("session"));

    accept_and_refresh(&summary_params(
        &db.pool,
        &session,
        &user,
        EVENT_POST_TOOL_USE,
    ))
    .await;

    let row = read_summary(&db.pool, session.as_str()).await;
    assert!(row.unique_files_touched.is_none());
    db.cleanup().await;
}

struct SessionSummaryParams<'a> {
    pool: &'a sqlx::PgPool,
    session_id: &'a SessionId,
    user_id: &'a UserId,
    event_type: &'a str,
    content_input_bytes: i64,
    content_output_bytes: i64,
    is_subagent_stop: bool,
    file_path: Option<&'a str>,
    is_from_subagent: bool,
}
async fn accept_and_refresh(p: &SessionSummaryParams<'_>) {
    let event = if p.is_subagent_stop {
        "SubagentStop"
    } else {
        p.event_type
    };
    let metadata =
        serde_json::json!({ "agent_id": if p.is_from_subagent { "subagent" } else { "" } });
    let dedup_key = unique("event");
    let plugin_id = systemprompt::identifiers::PluginId::new("test");
    let accepted = webhook::insert_plugin_usage_event(
        p.pool,
        &webhook::UsageEventParams {
            plugin_id: &plugin_id,
            user_id: p.user_id,
            session_id: p.session_id,
            event_type: event,
            tool_name: None,
            metadata: &metadata,
            description: None,
            prompt_preview: None,
            cwd: None,
            dedup_key: &dedup_key,
            content_input_bytes: p.content_input_bytes,
            content_output_bytes: p.content_output_bytes,
            loc_added: 0,
            loc_removed: 0,
        },
    )
    .await
    .expect("accept event");
    assert!(accepted, "a fresh dedup key is accepted");
    // Why: on this instance the summary is projected from the ingestion
    // outbox, which the refresh drains, rather than by the insert itself.
    usage_aggregations::refresh_session_summary(p.pool, p.session_id, p.file_path).await;
}
