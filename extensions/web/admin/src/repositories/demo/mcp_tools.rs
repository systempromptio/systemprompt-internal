//! Per-invocation MCP tool rows, their governance rollup, and the user matrix.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

use super::filter::DemoFilter;
use super::skill_matrix::UsageMatrix;

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

#[derive(Debug, Clone, Default)]
pub struct McpToolStatRow {
    pub server: String,
    pub tool: String,
    pub call_count: i64,
    pub failure_count: i64,
    pub failure_rate: f64,
    pub distinct_users: i64,
    pub allowed: i64,
    pub denied: i64,
    pub held: i64,
    pub approved: i64,
    pub rejected: i64,
    pub approval_pending: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub async fn list_mcp_tool_invocations(
    _pool: &PgPool,
    _filter: &DemoFilter,
) -> Result<Vec<McpToolInvocationRow>, sqlx::Error> {
    Ok(Vec::new())
}

pub async fn list_mcp_tool_stats(
    _pool: &PgPool,
    _filter: &DemoFilter,
) -> Result<Vec<McpToolStatRow>, sqlx::Error> {
    Ok(Vec::new())
}

pub async fn list_user_mcp_tool_matrix(
    _pool: &PgPool,
    _filter: &DemoFilter,
) -> Result<UsageMatrix, sqlx::Error> {
    Ok(UsageMatrix::default())
}
