//! Stat-strip totals for the sessions list.
//!
//! Aggregates the same `FULL OUTER JOIN` the list query pages over, under the
//! same filter, so the strip always describes the rows below it rather than
//! the whole table.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use super::SessionListFilter;
use crate::util::time_range::TimeRange;

#[derive(Debug, Clone, Copy, Default)]
pub struct SessionListKpis {
    pub total_sessions: i64,
    pub error_sessions: i64,
    pub total_requests: i64,
    pub total_tool_uses: i64,
    pub total_tokens: i64,
    pub total_cost_microdollars: i64,
}

pub async fn get_session_list_kpis(
    pool: &PgPool,
    filter: &SessionListFilter,
    range: TimeRange,
) -> Result<SessionListKpis, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        WITH req AS (
            SELECT
                session_id,
                MAX(user_id)                                       AS user_id,
                COUNT(*)::bigint                                   AS request_count,
                COUNT(*) FILTER (WHERE status = 'failed')::bigint  AS error_count,
                COALESCE(SUM(input_tokens), 0)::bigint             AS total_input_tokens,
                COALESCE(SUM(output_tokens), 0)::bigint            AS total_output_tokens,
                COALESCE(SUM(cost_microdollars), 0)::bigint        AS total_cost_microdollars,
                MIN(created_at)                                    AS first_seen,
                MAX(created_at)                                    AS last_seen
            FROM ai_requests
            WHERE session_id IS NOT NULL
            GROUP BY session_id
        ),
        joined AS (
            SELECT
                COALESCE(s.user_id, req.user_id)                    AS user_id,
                GREATEST(
                    COALESCE(req.last_seen, s.ended_at, s.started_at),
                    COALESCE(s.ended_at, s.started_at, req.last_seen)
                )                                                   AS last_activity_at,
                COALESCE(req.request_count, 0)                      AS request_count,
                COALESCE(s.tool_uses, 0)                            AS tool_uses,
                COALESCE(req.error_count, 0) + COALESCE(s.errors, 0) AS error_count,
                COALESCE(req.total_input_tokens, 0)
                    + COALESCE(s.total_input_tokens, 0)
                    + COALESCE(req.total_output_tokens, 0)
                    + COALESCE(s.total_output_tokens, 0)            AS total_tokens,
                COALESCE(req.total_cost_microdollars, 0)            AS total_cost_microdollars
            FROM plugin_session_summaries s
            FULL OUTER JOIN req ON req.session_id = s.session_id
        )
        SELECT
            COUNT(*)::bigint                                    AS "total_sessions!",
            COUNT(*) FILTER (WHERE error_count > 0)::bigint     AS "error_sessions!",
            COALESCE(SUM(request_count), 0)::bigint             AS "total_requests!",
            COALESCE(SUM(tool_uses), 0)::bigint                 AS "total_tool_uses!",
            COALESCE(SUM(total_tokens), 0)::bigint              AS "total_tokens!",
            COALESCE(SUM(total_cost_microdollars), 0)::bigint   AS "total_cost_microdollars!"
        FROM joined
        WHERE last_activity_at >= $1
          AND last_activity_at < $2
          AND ($3::text IS NULL OR user_id = $3)
          AND (NOT $4 OR error_count > 0)
        "#,
        range.from,
        range.to,
        filter.user_id.as_ref().map(UserId::as_str),
        filter.error_only,
    )
    .fetch_one(pool)
    .await?;

    Ok(SessionListKpis {
        total_sessions: row.total_sessions,
        error_sessions: row.error_sessions,
        total_requests: row.total_requests,
        total_tool_uses: row.total_tool_uses,
        total_tokens: row.total_tokens,
        total_cost_microdollars: row.total_cost_microdollars,
    })
}
