//! Per-invocation skill rows and their per-skill rollup.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

use super::filter::DemoFilter;

#[derive(Debug, Clone)]
pub struct SkillInvocationRow {
    pub user_id: UserId,
    pub user_email: Option<String>,
    pub session_id: SessionId,
    pub skill: String,
    pub plugin_id: Option<String>,
    pub tool_use_id: Option<String>,
    pub invoked_at: DateTime<Utc>,
    pub request_count: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
}

#[derive(Debug, Clone)]
pub struct SkillTotalRow {
    pub skill: String,
    pub invocation_count: i64,
    pub distinct_users: i64,
    pub request_count: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
    pub first_used_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub async fn list_skill_invocations(
    _pool: &PgPool,
    _filter: &DemoFilter,
) -> Result<Vec<SkillInvocationRow>, sqlx::Error> {
    Ok(Vec::new())
}

pub async fn list_skill_totals(
    _pool: &PgPool,
    _filter: &DemoFilter,
) -> Result<Vec<SkillTotalRow>, sqlx::Error> {
    Ok(Vec::new())
}
