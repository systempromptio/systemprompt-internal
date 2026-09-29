//! Who called an MCP server, and the raw call log behind it.
//!
//! Both read `mcp_tool_executions` directly, where a row is one call. The
//! callers roll it up per person, so the figure agrees with the Calls tile;
//! the call log keeps every row and says on each which side recorded it and
//! whether the client's completion hook attested it (`ai_tool_call_id`).

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{ContextId, McpExecutionId, SessionId, UserId};

#[derive(Debug, Clone)]
pub struct McpCallerRow {
    pub user_id: UserId,
    pub user_label: String,
    pub calls: i64,
    pub failures: i64,
    pub distinct_tools: i64,
    pub last_call_at: Option<DateTime<Utc>>,
}

// Why: Who has been using one server in the window, heaviest first; a row
// is a call, so the figure agrees with the Calls tile.
pub async fn list_mcp_callers(
    pool: &PgPool,
    server_name: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<McpCallerRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT
             x.user_id AS "user_id!: UserId",
             COALESCE(u.display_name, u.full_name, u.name, u.email, x.user_id) AS "user_label!",
             COUNT(*)::BIGINT AS "calls!",
             COUNT(*) FILTER (WHERE x.status IN ('failed', 'timeout'))::BIGINT AS "failures!",
             COUNT(DISTINCT x.tool_name)::BIGINT AS "distinct_tools!",
             MAX(x.started_at) AS "last_call_at?"
           FROM mcp_tool_executions x
           LEFT JOIN users u ON u.id = x.user_id
          WHERE x.server_name = $1
            AND x.started_at >= $2 AND x.started_at < $3
          GROUP BY x.user_id, u.display_name, u.full_name, u.name, u.email
          ORDER BY COUNT(*) DESC, MAX(x.started_at) DESC
          LIMIT $4"#,
        server_name,
        from,
        to,
        limit
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| McpCallerRow {
            user_id: r.user_id,
            user_label: r.user_label,
            calls: r.calls,
            failures: r.failures,
            distinct_tools: r.distinct_tools,
            last_call_at: r.last_call_at,
        })
        .collect())
}

#[derive(Debug, Clone)]
pub struct McpExecutionRow {
    pub execution_id: McpExecutionId,
    pub server_name: String,
    pub tool_name: String,
    pub source: String,
    pub correlation: String,
    pub status: String,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub execution_time_ms: Option<i32>,
    pub user_id: UserId,
    pub user_label: String,
    pub session_id: Option<SessionId>,
    pub context_id: Option<ContextId>,
    pub trace_id: Option<String>,
    pub ai_tool_call_id: Option<String>,
    pub input: String,
    pub error_message: Option<String>,
    pub payload_bytes: Option<i64>,
    pub secret_redactions: Option<i64>,
}

// Why: one call log ask — which server (`None` is the fleet, which only the
// export asks for), which window, and which page of it.
#[derive(Debug, Clone, Copy)]
pub struct CallLogQuery<'a> {
    pub server_name: Option<&'a str>,
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    pub limit: i64,
    pub offset: i64,
}

// Why: The raw call log in the window, newest first — the `source` column
// says which vantage point recorded the call, and `ai_tool_call_id` whether
// the client attested it.
pub async fn list_mcp_executions_paged(
    pool: &PgPool,
    q: CallLogQuery<'_>,
) -> Result<(Vec<McpExecutionRow>, i64), sqlx::Error> {
    let total = count_mcp_executions(pool, q).await?;
    let rows = page_mcp_executions(pool, q).await?;
    Ok((rows, total))
}

async fn count_mcp_executions(pool: &PgPool, q: CallLogQuery<'_>) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar!(
        r#"SELECT COUNT(*)::BIGINT AS "n!"
             FROM mcp_tool_executions
            WHERE ($1::TEXT IS NULL OR server_name = $1)
              AND started_at >= $2 AND started_at < $3"#,
        q.server_name,
        q.from,
        q.to
    )
    .fetch_one(pool)
    .await
}

async fn page_mcp_executions(
    pool: &PgPool,
    q: CallLogQuery<'_>,
) -> Result<Vec<McpExecutionRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT
             x.mcp_execution_id AS "execution_id!: McpExecutionId",
             x.server_name AS "server_name!",
             x.tool_name AS "tool_name!",
             x.source AS "source!",
             x.correlation AS "correlation!",
             x.status AS "status!",
             x.started_at AS "started_at!",
             x.completed_at AS "completed_at?",
             x.execution_time_ms AS "execution_time_ms?",
             x.user_id AS "user_id!: UserId",
             COALESCE(u.display_name, u.full_name, u.name, u.email, x.user_id) AS "user_label!",
             x.session_id AS "session_id?: SessionId",
             x.context_id AS "context_id?: ContextId",
             x.trace_id AS "trace_id?",
             x.ai_tool_call_id AS "ai_tool_call_id?",
             x.input AS "input!",
             x.error_message AS "error_message?",
             a.payload_bytes::BIGINT AS "payload_bytes?",
             a.secret_redactions::BIGINT AS "secret_redactions?"
           FROM mcp_tool_executions x
           LEFT JOIN users u ON u.id = x.user_id
           LEFT JOIN mcp_artifacts a ON a.mcp_execution_id = x.mcp_execution_id
          WHERE ($1::TEXT IS NULL OR x.server_name = $1)
            AND x.started_at >= $2 AND x.started_at < $3
          ORDER BY x.started_at DESC
          LIMIT $4 OFFSET $5"#,
        q.server_name,
        q.from,
        q.to,
        q.limit,
        q.offset
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| McpExecutionRow {
            execution_id: r.execution_id,
            server_name: r.server_name,
            tool_name: r.tool_name,
            source: r.source,
            correlation: r.correlation,
            status: r.status,
            started_at: r.started_at,
            completed_at: r.completed_at,
            execution_time_ms: r.execution_time_ms,
            user_id: r.user_id,
            user_label: r.user_label,
            session_id: r.session_id,
            context_id: r.context_id,
            trace_id: r.trace_id,
            ai_tool_call_id: r.ai_tool_call_id,
            input: r.input,
            error_message: r.error_message,
            payload_bytes: r.payload_bytes,
            secret_redactions: r.secret_redactions,
        })
        .collect())
}
