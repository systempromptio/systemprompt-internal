//! Headline figures for the dashboard's KPI strip.
//!
//! The token total here must match the profile pane's exactly. The canonical
//! definition, and why it is what it is, lives in
//! [`crate::repositories::users::usage`]; the two are pinned together by
//! `tests/integration/admin-core/src/usage_reconciliation.rs`.

use sqlx::PgPool;

use crate::util::time_range::TimeRange;

use super::SiteScope;

#[derive(Debug, Default, Clone, Copy)]
pub struct SiteKpis {
    pub total_requests: i64,
    pub error_count: i64,
    pub total_cost_microdollars: i64,
    pub total_tokens: i64,
    // Why: reasoning is billed inside `output_tokens` and inside
    // `tokens_used`; it is reported beside them as a share, never added.
    pub reasoning_tokens: i64,
    pub output_tokens: i64,
    pub active_users: i64,
    // Why: Distinct users with at least one request in the trailing 7 days —
    // computed against `NOW()` regardless of the picked window, and labeled
    // that way on the page.
    pub weekly_active_users: i64,
    // Why: The same aggregates over the immediately preceding window of equal
    // width (`[from - (to-from), from)`), read in the same statement so both
    // windows see one snapshot and a delta can never be skewed by writes
    // landing between two queries.
    pub prev_total_requests: i64,
    pub prev_error_count: i64,
    pub prev_total_cost_microdollars: i64,
    pub prev_total_tokens: i64,
    pub prev_active_users: i64,
    // Why: WAU for the prior trailing week (`NOW()-14d .. NOW()-7d`), anchored to
    // the same `NOW()` as `weekly_active_users`.
    pub prev_weekly_active_users: i64,
}

// Why: current and previous window are counted in one pass with `FILTER`
// clauses rather than two round trips, so both totals see exactly the same
// snapshot of `ai_requests` and a request landing mid-read cannot be counted
// in one window and missed in the other.
pub async fn get_site_kpis(
    pool: &PgPool,
    range: TimeRange,
    scope: &SiteScope,
) -> Result<SiteKpis, sqlx::Error> {
    // Why: the previous window's edge is computed here, not in SQL, so the
    // two windows are guaranteed the same width to the microsecond.
    let prev_from = range.from - (range.to - range.from);
    let row = sqlx::query_file!(
        "src/repositories/analytics/site/kpis.sql",
        range.from,
        range.to,
        scope.scope.as_sql(),
        scope.user_id_str(),
        prev_from,
    )
    .fetch_one(pool)
    .await?;

    Ok(SiteKpis {
        total_requests: row.total,
        error_count: row.errors,
        total_cost_microdollars: row.cost,
        total_tokens: row.tokens,
        reasoning_tokens: row.reasoning_tokens,
        output_tokens: row.output_tokens,
        active_users: row.active_users,
        weekly_active_users: row.weekly_active_users,
        prev_total_requests: row.prev_total,
        prev_error_count: row.prev_errors,
        prev_total_cost_microdollars: row.prev_cost,
        prev_total_tokens: row.prev_tokens,
        prev_active_users: row.prev_active_users,
        prev_weekly_active_users: row.prev_weekly_active_users,
    })
}
