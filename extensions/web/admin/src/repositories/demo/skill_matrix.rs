//! User × entity usage matrix, shared by the skill and MCP tool pages.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use super::filter::DemoFilter;

#[derive(Debug, Clone, Default)]
pub struct UsageMatrix {
    pub columns: Vec<String>,
    pub rows: Vec<UsageMatrixRow>,
}

#[derive(Debug, Clone)]
pub struct UsageMatrixRow {
    pub user_id: UserId,
    pub user_email: Option<String>,
    pub cells: Vec<i64>,
    pub total: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
}

pub async fn list_user_skill_matrix(
    _pool: &PgPool,
    _filter: &DemoFilter,
) -> Result<UsageMatrix, sqlx::Error> {
    Ok(UsageMatrix::default())
}
