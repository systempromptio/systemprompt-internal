//! One row per MCP tool call, with attributed AI usage.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

use crate::repositories::demo::attribution::ATTRIBUTION_PAD_MINUTES;
use crate::repositories::demo::filter::DemoFilter;

#[derive(Debug, Clone)]
pub struct McpToolInvocationRow {
    pub user_id: UserId,
    pub user_email: Option<String>,
    pub session_id: SessionId,
    pub server: String,
    pub tool: String,
    pub tool_name: String,
    pub plugin_id: Option<String>,
    pub tool_use_id: Option<String>,
    pub is_failure: bool,
    pub invoked_at: DateTime<Utc>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
}

pub async fn list_mcp_tool_invocations(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<Vec<McpToolInvocationRow>, sqlx::Error> {
    sqlx::query_as!(
        McpToolInvocationRow,
        r#"
        WITH ev AS (
            SELECT user_id, session_id, created_at, event_type, tool_name, metadata, plugin_id
            FROM plugin_usage_events
            WHERE created_at >= $1
              AND ($2::text IS NULL OR user_id = $2)
        ),
        session_bounds AS (
            SELECT session_id, MAX(created_at) AS last_at FROM ev GROUP BY session_id
        ),
        inv AS (
            SELECT
                e.user_id,
                e.session_id,
                e.plugin_id,
                e.tool_name,
                split_part(e.tool_name, '__', 2) AS server,
                substr(
                    e.tool_name,
                    length('mcp__' || split_part(e.tool_name, '__', 2) || '__') + 1
                ) AS tool,
                e.metadata->>'tool_use_id' AS tool_use_id,
                (e.event_type = 'PostToolUseFailure') AS is_failure,
                e.created_at AS invoked_at,
                LEAD(e.created_at) OVER (
                    PARTITION BY e.session_id ORDER BY e.created_at
                ) AS next_at
            FROM ev e
            WHERE e.tool_name LIKE 'mcp\_\_%'
              AND e.event_type IN ('PostToolUse', 'PostToolUseFailure')
        ),
        bounded AS (
            SELECT i.*,
                   LEAST(
                       COALESCE(i.next_at, 'infinity'::timestamptz),
                       b.last_at + make_interval(mins => $3::int)
                   ) AS window_end
            FROM inv i
            JOIN session_bounds b ON b.session_id = i.session_id
        )
        SELECT
            bd.user_id      AS "user_id!: UserId",
            u.email         AS "user_email?",
            bd.session_id   AS "session_id!: SessionId",
            bd.server       AS "server!",
            bd.tool         AS "tool!",
            bd.tool_name    AS "tool_name!",
            bd.plugin_id    AS "plugin_id?",
            bd.tool_use_id  AS "tool_use_id?",
            bd.is_failure   AS "is_failure!",
            bd.invoked_at   AS "invoked_at!",
            a.request_count      AS "request_count!",
            a.total_tokens       AS "total_tokens!",
            a.cost_microdollars  AS "cost_microdollars!"
        FROM bounded bd
        LEFT JOIN users u ON u.id = bd.user_id
        LEFT JOIN LATERAL (
            SELECT
                COUNT(*)::bigint AS request_count,
                COALESCE(SUM(COALESCE(r.input_tokens, 0) + COALESCE(r.output_tokens, 0)), 0)::bigint
                    AS total_tokens,
                COALESCE(SUM(r.cost_microdollars), 0)::bigint AS cost_microdollars
            FROM ai_requests r
            WHERE r.user_id = bd.user_id
              AND r.created_at >= bd.invoked_at AND r.created_at < bd.window_end
        ) a ON TRUE
        ORDER BY bd.invoked_at DESC
        LIMIT $4
        "#,
        filter.since,
        filter.user_filter(),
        ATTRIBUTION_PAD_MINUTES,
        filter.limit,
    )
    .fetch_all(pool)
    .await
}
