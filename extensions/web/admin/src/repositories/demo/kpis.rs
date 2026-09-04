//! The KPI strip shared by all four demo pages.
//!
//! `allowed` counts real tool verdicts only; see [`super::policy`] for why the
//! per-request server authorization rows are excluded by shape rather than by
//! policy name.

use serde::Serialize;
use sqlx::PgPool;

use super::attribution::ATTRIBUTION_PAD_MINUTES;
use super::filter::DemoFilter;

#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct DemoKpis {
    pub skill_invocations: i64,
    pub allowed: i64,
    pub mcp_calls: i64,
    pub mcp_failures: i64,
    pub held: i64,
    pub refused: i64,
    pub blocked: i64,
    pub approved: i64,
    pub attributed_tokens: i64,
    pub attributed_cost_microdollars: i64,
}

pub async fn get_demo_kpis(pool: &PgPool, filter: &DemoFilter) -> Result<DemoKpis, sqlx::Error> {
    let (events, decisions, usage) = tokio::join!(
        get_event_counts(pool, filter),
        get_decision_counts(pool, filter),
        get_attributed_usage(pool, filter),
    );
    let events = events?;
    let decisions = decisions?;
    let usage = usage?;

    Ok(DemoKpis {
        skill_invocations: events.0,
        allowed: decisions.4,
        mcp_calls: events.1,
        mcp_failures: events.2,
        held: decisions.0,
        refused: decisions.1,
        blocked: decisions.2,
        approved: decisions.3,
        attributed_tokens: usage.0,
        attributed_cost_microdollars: usage.1,
    })
}

async fn get_event_counts(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<(i64, i64, i64), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
            (
                SELECT COUNT(*)::bigint
                FROM skill_invocation_events v
                WHERE v.invoked_at >= $1
                  AND ($2::text IS NULL OR v.user_id = $2)
            ) AS "skills!",
            COUNT(*) FILTER (WHERE tool_name LIKE 'mcp\_\_%')::bigint AS "mcp_calls!",
            COUNT(*) FILTER (
                WHERE tool_name LIKE 'mcp\_\_%' AND event_type = 'PostToolUseFailure'
            )::bigint AS "mcp_failures!"
        FROM plugin_usage_events
        WHERE created_at >= $1
          AND ($2::text IS NULL OR user_id = $2)
          AND event_type IN ('PostToolUse', 'PostToolUseFailure')
        "#,
        filter.since,
        filter.user_filter(),
    )
    .fetch_one(pool)
    .await?;
    Ok((row.skills, row.mcp_calls, row.mcp_failures))
}

async fn get_decision_counts(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<(i64, i64, i64, i64, i64), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
            (SELECT COUNT(*) FROM governance_decisions g
              WHERE g.created_at >= $1 AND ($2::text IS NULL OR g.user_id = $2)
                AND g.decision = 'pending')::bigint AS "held!",
            (SELECT COUNT(*) FROM governance_decisions g
              WHERE g.created_at >= $1 AND ($2::text IS NULL OR g.user_id = $2)
                AND g.decision = 'deny' AND g.policy = 'secret_scan')::bigint AS "refused!",
            (SELECT COUNT(*) FROM governance_decisions g
              WHERE g.created_at >= $1 AND ($2::text IS NULL OR g.user_id = $2)
                AND g.decision = 'deny' AND g.policy = 'tool_blocklist')::bigint AS "blocked!",
            (SELECT COUNT(*) FROM approval_requests a
              WHERE a.created_at >= $1 AND ($2::text IS NULL OR a.requested_by = $2)
                AND a.status = 'approved')::bigint AS "approved!",
            (SELECT COUNT(*) FROM governance_decisions g
              WHERE g.created_at >= $1 AND ($2::text IS NULL OR g.user_id = $2)
                AND g.decision = 'allow'
                AND g.policy <> 'authz' AND NOT (g.policy = 'authz_rule_based' AND g.plugin_id IS NULL))::bigint
                AS "allowed!"
        "#,
        filter.since,
        filter.user_filter(),
    )
    .fetch_one(pool)
    .await?;
    Ok((
        row.held,
        row.refused,
        row.blocked,
        row.approved,
        row.allowed,
    ))
}

async fn get_attributed_usage(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<(i64, i64), sqlx::Error> {
    let row = sqlx::query!(
        r#"
        WITH ev AS (
            SELECT user_id, session_id, created_at, event_type, tool_name, metadata
            FROM plugin_usage_events
            WHERE created_at >= $1
              AND ($2::text IS NULL OR user_id = $2)
        ),
        session_bounds AS (
            SELECT session_id, MAX(created_at) AS last_at FROM ev GROUP BY session_id
        ),
        inv AS (
            SELECT
                e.user_id,
                e.session_id,
                e.created_at AS invoked_at,
                LEAD(e.created_at) OVER (
                    PARTITION BY e.session_id ORDER BY e.created_at
                ) AS next_at
            FROM ev e
            WHERE (
                    e.event_type IN ('PostToolUse', 'PostToolUseFailure')
                AND e.tool_name LIKE 'mcp\_\_%'
              )
               OR EXISTS (
                    SELECT 1 FROM skill_invocation_events v
                    WHERE v.session_id = e.session_id
                      AND v.invoked_at = e.created_at
              )
        ),
        windows AS (
            SELECT i.user_id, i.invoked_at,
                   LEAST(
                       COALESCE(i.next_at, 'infinity'::timestamptz),
                       b.last_at + make_interval(mins => $3::int)
                   ) AS window_end
            FROM inv i
            JOIN session_bounds b ON b.session_id = i.session_id
        )
        SELECT
            COALESCE(SUM(COALESCE(r.input_tokens, 0) + COALESCE(r.output_tokens, 0)), 0)::bigint
                AS "tokens!",
            COALESCE(SUM(r.cost_microdollars), 0)::bigint AS "cost!"
        FROM ai_requests r
        WHERE ($2::text IS NULL OR r.user_id = $2)
          AND r.created_at >= $1
          AND EXISTS (
              SELECT 1 FROM windows w
              WHERE w.user_id = r.user_id
                AND r.created_at >= w.invoked_at
                AND r.created_at <  w.window_end
          )
        "#,
        filter.since,
        filter.user_filter(),
        ATTRIBUTION_PAD_MINUTES,
    )
    .fetch_one(pool)
    .await?;
    Ok((row.tokens, row.cost))
}
