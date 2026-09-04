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
                v.invoked_at::date AS day,
                COUNT(*)::bigint AS count,
                -- Why: only a dispatched Skill tool call can fail as a tool. A
                -- slash command is a prompt, so it has no failure state and
                -- never lands in this bucket.
                COUNT(*) FILTER (
                    WHERE e.event_type = 'PostToolUseFailure'
                )::bigint AS failures
            FROM skill_invocation_events v
            LEFT JOIN plugin_usage_events e
                   ON e.session_id = v.session_id
                  AND e.created_at = v.invoked_at
                  AND e.tool_name = 'Skill'
            WHERE v.invoked_at >= (SELECT lo FROM span)
              AND ($1::text IS NULL OR v.user_id = $1)
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
