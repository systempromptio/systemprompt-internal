//! The live-sessions board and per-workspace cost attribution.
//!
//! Liveness is `last_event_at` within a window rather than `ended_at IS NULL`:
//! a session that crashed or had its laptop closed never sends `SessionEnd`,
//! so on the `ended_at` test alone it would appear live forever.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

pub const DEFAULT_LIVE_WINDOW_MINUTES: i32 = 15;

#[derive(Debug, Clone)]
pub struct LiveSessionRow {
    pub session_id: SessionId,
    pub user_id: UserId,
    pub display_name: Option<String>,
    pub handle: Option<String>,
    pub workspace: Option<String>,
    pub git_branch: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub current_activity: Option<String>,
    pub permission_mode: Option<String>,
    pub client_source: Option<String>,
    pub tool_uses: Option<i64>,
    pub prompts: Option<i64>,
    pub errors: Option<i64>,
    pub live_cost_microdollars: Option<i64>,
    pub context_pct: Option<i16>,
    pub started_at: Option<DateTime<Utc>>,
    pub last_event_at: Option<DateTime<Utc>>,
}

pub async fn list_live_sessions(
    pool: &PgPool,
    window_minutes: i32,
    limit: i64,
) -> Result<Vec<LiveSessionRow>, sqlx::Error> {
    sqlx::query_as!(
        LiveSessionRow,
        r#"SELECT s.session_id AS "session_id: SessionId",
                  s.user_id AS "user_id: UserId",
                  u.display_name,
                  s.handle,
                  s.workspace,
                  s.git_branch,
                  s.cwd,
                  s.model,
                  s.current_activity,
                  s.permission_mode,
                  s.client_source,
                  s.tool_uses,
                  s.prompts,
                  s.errors,
                  s.live_cost_microdollars,
                  s.context_pct,
                  s.started_at,
                  s.last_event_at
           FROM plugin_session_summaries s
           LEFT JOIN users u ON u.id = s.user_id
           WHERE s.ended_at IS NULL
             AND s.last_event_at IS NOT NULL
             AND s.last_event_at > NOW() - make_interval(mins => $1)
           ORDER BY s.last_event_at DESC
           LIMIT $2"#,
        window_minutes,
        limit,
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone)]
pub struct WorkspaceCostRow {
    pub workspace: Option<String>,
    pub session_count: Option<i64>,
    pub request_count: Option<i64>,
    pub total_cost_microdollars: Option<i64>,
    pub total_input_tokens: Option<i64>,
    pub total_output_tokens: Option<i64>,
}

// Why: `ai_requests.session_id` FKs to core's web `user_sessions`, not to
// `plugin_session_summaries`, so this join is the only path from "what an agent
// session was working on" to "what it cost". Agent sessions that never issued
// an inference simply do not match, which is why the cost columns coalesce.
pub async fn list_workspace_costs(
    pool: &PgPool,
    since: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<WorkspaceCostRow>, sqlx::Error> {
    sqlx::query_as!(
        WorkspaceCostRow,
        r"SELECT s.workspace,
                 COUNT(DISTINCT s.session_id) AS session_count,
                 COUNT(r.id) AS request_count,
                 COALESCE(SUM(r.cost_microdollars), 0)::BIGINT AS total_cost_microdollars,
                 COALESCE(SUM(r.input_tokens), 0)::BIGINT AS total_input_tokens,
                 COALESCE(SUM(r.output_tokens), 0)::BIGINT AS total_output_tokens
          FROM plugin_session_summaries s
          LEFT JOIN ai_requests r
                 ON r.session_id = s.session_id
                AND r.created_at >= $1
          WHERE s.workspace IS NOT NULL
            AND s.started_at >= $1
          GROUP BY s.workspace
          ORDER BY total_cost_microdollars DESC
          LIMIT $2",
        since,
        limit,
    )
    .fetch_all(pool)
    .await
}
