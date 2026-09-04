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
    pool: &PgPool,
    filter: &DemoFilter,
    days: i32,
) -> Result<Vec<DailyBucket>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        WITH span AS (
            SELECT (date_trunc('day', now()) - make_interval(days => $2::int - 1)) AS lo
        ),
        calendar AS (
            SELECT generate_series((SELECT lo FROM span), date_trunc('day', now()),
                                   interval '1 day')::date AS day
        ),
        counted AS (
            SELECT
                created_at::date AS day,
                COUNT(*)::bigint AS count,
                COUNT(*) FILTER (WHERE event_type = 'PostToolUseFailure')::bigint AS failures
            FROM plugin_usage_events
            WHERE created_at >= (SELECT lo FROM span)
              AND ($1::text IS NULL OR user_id = $1)
              AND tool_name = 'Skill'
              AND event_type IN ('PostToolUse', 'PostToolUseFailure')
              AND metadata->'tool_input'->>'skill' IS NOT NULL
            GROUP BY 1
        )
        SELECT
            c.day                          AS "day!",
            COALESCE(t.count, 0)::bigint    AS "count!",
            COALESCE(t.failures, 0)::bigint AS "failures!"
        FROM calendar c
        LEFT JOIN counted t ON t.day = c.day
        ORDER BY c.day
        "#,
        filter.user_filter(),
        days,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| DailyBucket {
            day: r.day,
            count: r.count,
            failures: r.failures,
        })
        .collect())
}

pub async fn list_mcp_tool_daily_series(
    pool: &PgPool,
    filter: &DemoFilter,
    days: i32,
) -> Result<Vec<DailyBucket>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        WITH span AS (
            SELECT (date_trunc('day', now()) - make_interval(days => $2::int - 1)) AS lo
        ),
        calendar AS (
            SELECT generate_series((SELECT lo FROM span), date_trunc('day', now()),
                                   interval '1 day')::date AS day
        ),
        counted AS (
            SELECT
                created_at::date AS day,
                COUNT(*)::bigint AS count,
                COUNT(*) FILTER (WHERE event_type = 'PostToolUseFailure')::bigint AS failures
            FROM plugin_usage_events
            WHERE created_at >= (SELECT lo FROM span)
              AND ($1::text IS NULL OR user_id = $1)
              AND tool_name LIKE 'mcp\_\_%'
              AND event_type IN ('PostToolUse', 'PostToolUseFailure')
            GROUP BY 1
        )
        SELECT
            c.day                          AS "day!",
            COALESCE(t.count, 0)::bigint    AS "count!",
            COALESCE(t.failures, 0)::bigint AS "failures!"
        FROM calendar c
        LEFT JOIN counted t ON t.day = c.day
        ORDER BY c.day
        "#,
        filter.user_filter(),
        days,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| DailyBucket {
            day: r.day,
            count: r.count,
            failures: r.failures,
        })
        .collect())
}
