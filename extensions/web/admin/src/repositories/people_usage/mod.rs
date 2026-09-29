//! Gateway usage rolled up per people container — a group or a project.
//!
//! Every statement here is built on the shared CTEs in
//! `repositories::scope::membership`, so which relation names the people in a
//! container, and whether a request counts once or in every container its
//! person belongs to, are bound parameters rather than interpolated text.
//! Spend joins `request_scope`, the container stamped on each request when it
//! landed; membership is only what puts a person with no traffic on the list.
//!
//! Group membership reads the `user_groups` view, so the derived `unassigned`
//! bucket rolls up exactly like a real group.
//!
//! `tokens` is the provider's own count when it reported one; the four
//! component columns are the fallback for rows written before it existed.

pub mod attribution;
pub mod breakdown;
pub mod totals;

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::repositories::scope::ScopeQuery;

// Why: The window every page and endpoint defaults to.
pub const DEFAULT_WINDOW_DAYS: i32 = 30;

// Why: How many rows a leaderboard carries. Ten is what the screens draw.
pub const LEADERBOARD_LIMIT: i64 = 10;

/// One container's thirty-day totals, keyed back to the container.
#[derive(Debug, Clone, Default, Serialize)]
pub struct ScopeUsageRow {
    pub scope_id: String,
    pub active_members: i64,
    pub requests: i64,
    pub tokens: i64,
    pub tokens_in: i64,
    pub tokens_out: i64,
    pub cost_microdollars: i64,
}

/// What one member of a container spent inside it.
#[derive(Debug, Clone)]
pub struct MemberUsageRow {
    pub user_id: UserId,
    pub requests: i64,
    pub tokens: i64,
    pub cost_microdollars: i64,
    pub last_active: Option<DateTime<Utc>>,
}

/// One day's request and cost total, gap-filled so a chart's spine is the
/// calendar rather than the days that happened to carry traffic.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct DailyRequests {
    pub day: NaiveDate,
    pub requests: i64,
    pub cost_microdollars: i64,
}

pub async fn get_scope_usage(
    pool: &PgPool,
    q: &ScopeQuery<'_>,
) -> Result<ScopeUsageRow, sqlx::Error> {
    let row = crate::scoped_query!(
        r#"SELECT COUNT(r.id)::BIGINT AS "requests!",
                  COUNT(DISTINCT r.user_id)::BIGINT AS "active_members!",
                  COALESCE(SUM(COALESCE(r.tokens_used, COALESCE(r.input_tokens, 0) + COALESCE(r.output_tokens, 0) + COALESCE(r.cache_read_tokens, 0) + COALESCE(r.cache_creation_tokens, 0))), 0)::BIGINT AS "tokens!",
                  COALESCE(SUM(r.input_tokens), 0)::BIGINT AS "tokens_in!",
                  COALESCE(SUM(r.output_tokens), 0)::BIGINT AS "tokens_out!",
                  COALESCE(SUM(r.cost_microdollars), 0)::BIGINT AS "cost_microdollars!"
           FROM request_scope rs
           JOIN ai_requests r
             ON r.id = rs.request_id
            AND r.created_at >= NOW() - make_interval(days => $4)
           WHERE rs.scope_id = $3"#,
        q.kind().as_str(),
        q.attribution.is_exclusive(),
        q.id(),
        q.window_days
    )
    .fetch_one(pool)
    .await?;
    Ok(ScopeUsageRow {
        scope_id: q.id().to_owned(),
        active_members: row.active_members,
        requests: row.requests,
        tokens: row.tokens,
        tokens_in: row.tokens_in,
        tokens_out: row.tokens_out,
        cost_microdollars: row.cost_microdollars,
    })
}

pub async fn list_member_usage(
    pool: &PgPool,
    q: &ScopeQuery<'_>,
) -> Result<HashMap<String, MemberUsageRow>, sqlx::Error> {
    // Why: the people are the current members plus anyone whose requests were
    // stamped to this container, so a person who has since moved still
    // accounts for the spend they left behind and the rows sum to the total.
    let rows = crate::scoped_query!(
        r#", spent AS (
               SELECT r.id, r.user_id, r.input_tokens, r.output_tokens,
                      r.cost_microdollars, r.created_at
               FROM request_scope rs
               JOIN ai_requests r ON r.id = rs.request_id
               WHERE rs.scope_id = $3
                 AND r.created_at >= NOW() - make_interval(days => $4)
           ), people AS (
               SELECT m.user_id FROM membership m WHERE m.scope_id = $3
               UNION
               SELECT sp.user_id FROM spent sp
           )
           SELECT pe.user_id AS "user_id!: UserId",
                  COUNT(r.id)::BIGINT AS "requests!",
                  COALESCE(SUM(COALESCE(r.input_tokens, 0) + COALESCE(r.output_tokens, 0)), 0)::BIGINT AS "tokens!",
                  COALESCE(SUM(r.cost_microdollars), 0)::BIGINT AS "cost_microdollars!",
                  MAX(r.created_at) AS "last_active?"
           FROM people pe
           LEFT JOIN spent r ON r.user_id = pe.user_id
           GROUP BY pe.user_id"#,
        q.kind().as_str(),
        q.attribution.is_exclusive(),
        q.id(),
        q.window_days
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.user_id.as_str().to_owned(),
                MemberUsageRow {
                    user_id: row.user_id,
                    requests: row.requests,
                    tokens: row.tokens,
                    cost_microdollars: row.cost_microdollars,
                    last_active: row.last_active,
                },
            )
        })
        .collect())
}

pub async fn list_daily_requests(
    pool: &PgPool,
    q: &ScopeQuery<'_>,
) -> Result<Vec<DailyRequests>, sqlx::Error> {
    let rows = crate::scoped_query!(
        r#", days AS (
               SELECT generate_series(
                   (NOW() - make_interval(days => $4 - 1))::date,
                   NOW()::date,
                   INTERVAL '1 day'
               )::date AS day
           ), counts AS (
               SELECT r.created_at::date AS day,
                      COUNT(*)::BIGINT AS requests,
                      COALESCE(SUM(r.cost_microdollars), 0)::BIGINT AS cost_microdollars
               FROM request_scope rs
               JOIN ai_requests r ON r.id = rs.request_id
               WHERE rs.scope_id = $3
                 AND r.created_at >= NOW() - make_interval(days => $4)
               GROUP BY 1
           )
           SELECT days.day AS "day!",
                  COALESCE(counts.requests, 0)::BIGINT AS "requests!",
                  COALESCE(counts.cost_microdollars, 0)::BIGINT AS "cost_microdollars!"
           FROM days LEFT JOIN counts ON counts.day = days.day
           ORDER BY days.day"#,
        q.kind().as_str(),
        q.attribution.is_exclusive(),
        q.id(),
        q.window_days
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| DailyRequests {
            day: row.day,
            requests: row.requests,
            cost_microdollars: row.cost_microdollars,
        })
        .collect())
}
