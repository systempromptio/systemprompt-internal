//! Merged chronological demo logbook: skills, MCP calls, decisions, approvals.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

use super::filter::DemoFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogbookKind {
    Skill,
    McpTool,
    Decision,
    Approval,
}

#[derive(Debug, Clone)]
pub struct LogbookRow {
    pub kind: LogbookKind,
    pub at: DateTime<Utc>,
    pub user_id: UserId,
    pub user_email: Option<String>,
    pub session_id: SessionId,
    pub label: String,
    pub detail: Option<String>,
    pub status: Option<String>,
    pub policy: Option<String>,
}

pub async fn list_demo_logbook(
    _pool: &PgPool,
    _filter: &DemoFilter,
    _include_allows: bool,
) -> Result<Vec<LogbookRow>, sqlx::Error> {
    Ok(Vec::new())
}
