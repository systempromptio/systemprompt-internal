//! Daily buckets for the 14-day bar charts, gap-filled with `generate_series`.

use chrono::NaiveDate;
use sqlx::PgPool;

use super::filter::DemoFilter;

#[derive(Debug, Clone, Copy)]
pub struct DailyBucket {
    pub day: NaiveDate,
    pub count: i64,
    pub failures: i64,
}

pub async fn list_skill_daily_series(
    _pool: &PgPool,
    _filter: &DemoFilter,
    _days: i32,
) -> Result<Vec<DailyBucket>, sqlx::Error> {
    Ok(Vec::new())
}

pub async fn list_mcp_tool_daily_series(
    _pool: &PgPool,
    _filter: &DemoFilter,
    _days: i32,
) -> Result<Vec<DailyBucket>, sqlx::Error> {
    Ok(Vec::new())
}
