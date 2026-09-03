//! Skill usage rollups — drives `/admin/entities/skills`.
//!
//! Skill invocations are already tracked as `session_entity_links` rows with
//! `entity_type = 'skill'` (written by
//! `handlers::hooks_track::processing::track_session_entity` on every `Skill`
//! tool call). This module aggregates those rows and joins them to
//! `ai_requests` by `session_id` for a cost/token estimate. That join is
//! session-scoped, not call-scoped — `PreToolUse` events are dropped, and
//! neither table carries a shared `trace_id` — so a session containing other
//! activity besides the skill call inflates the estimate. Callers must label
//! it as an estimate, not an exact per-invocation cost.

use chrono::{DateTime, Utc};
use sqlx::PgPool;

#[derive(Debug, Clone)]
pub struct SkillUsageRow {
    pub skill_id: String,
    pub invocation_count: i64,
    pub distinct_users: i64,
    pub first_used_at: Option<DateTime<Utc>>,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub async fn list_skill_usage_stats(pool: &PgPool) -> Result<Vec<SkillUsageRow>, sqlx::Error> {
    sqlx::query_as!(
        SkillUsageRow,
        r#"
        SELECT
            entity_name                          AS "skill_id!",
            COALESCE(SUM(usage_count), 0)::bigint AS "invocation_count!",
            COUNT(DISTINCT user_id)::bigint       AS "distinct_users!",
            MIN(first_seen_at)                    AS "first_used_at?",
            MAX(last_seen_at)                     AS "last_used_at?"
        FROM session_entity_links
        WHERE entity_type = 'skill'
        GROUP BY entity_name
        ORDER BY SUM(usage_count) DESC
        "#
    )
    .fetch_all(pool)
    .await
}

#[derive(Debug, Clone, Copy, Default)]
pub struct SkillCostEstimate {
    pub session_count: i64,
    pub request_count: i64,
    pub total_input_tokens: i64,
    pub total_output_tokens: i64,
    pub total_cost_microdollars: i64,
}

pub async fn get_skill_cost_estimate(
    pool: &PgPool,
    skill_id: &str,
) -> Result<SkillCostEstimate, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
            COUNT(DISTINCT r.session_id)::bigint        AS "session_count!",
            COUNT(*)::bigint                             AS "request_count!",
            COALESCE(SUM(r.input_tokens), 0)::bigint     AS "total_input_tokens!",
            COALESCE(SUM(r.output_tokens), 0)::bigint    AS "total_output_tokens!",
            COALESCE(SUM(r.cost_microdollars), 0)::bigint AS "total_cost_microdollars!"
        FROM ai_requests r
        WHERE r.session_id IN (
            SELECT session_id FROM session_entity_links
            WHERE entity_type = 'skill' AND entity_name = $1
        )
        "#,
        skill_id
    )
    .fetch_one(pool)
    .await?;
    Ok(SkillCostEstimate {
        session_count: row.session_count,
        request_count: row.request_count,
        total_input_tokens: row.total_input_tokens,
        total_output_tokens: row.total_output_tokens,
        total_cost_microdollars: row.total_cost_microdollars,
    })
}
