//! Per-invocation skill rows and their per-skill rollup.
//!
//! A skill invocation is a `plugin_usage_events` row with `tool_name = 'Skill'`
//! whose `metadata->'tool_input'->>'skill'` names the skill as `plugin:skill`.
//! Token and cost columns are attributed by the window rule in
//! [`super::attribution`], not measured.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

use super::attribution::ATTRIBUTION_PAD_MINUTES;
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
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<Vec<SkillInvocationRow>, sqlx::Error> {
    sqlx::query_as!(
        SkillInvocationRow,
        r#"
        WITH ev AS (
            SELECT user_id, session_id, created_at, tool_name, metadata, plugin_id
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
                e.plugin_id,
                e.metadata->'tool_input'->>'skill' AS skill,
                e.metadata->>'tool_use_id'         AS tool_use_id,
                e.created_at                       AS invoked_at,
                LEAD(e.created_at) OVER (
                    PARTITION BY e.session_id ORDER BY e.created_at
                ) AS next_at
            FROM ev e
            WHERE e.tool_name = 'Skill'
              AND e.metadata->'tool_input'->>'skill' IS NOT NULL
        ),
        bounded AS (
            SELECT i.*,
                   LEAST(
                       COALESCE(i.next_at, 'infinity'::timestamptz),
                       b.last_at + make_interval(mins => $3::int)
                   ) AS window_end
            FROM inv i
            JOIN session_bounds b ON b.session_id = i.session_id
        )
        SELECT
            bd.user_id      AS "user_id!: UserId",
            u.email         AS "user_email?",
            bd.session_id   AS "session_id!: SessionId",
            bd.skill        AS "skill!",
            bd.plugin_id    AS "plugin_id?",
            bd.tool_use_id  AS "tool_use_id?",
            bd.invoked_at   AS "invoked_at!",
            a.request_count      AS "request_count!",
            a.total_tokens       AS "total_tokens!",
            a.cost_microdollars  AS "cost_microdollars!"
        FROM bounded bd
        LEFT JOIN users u ON u.id = bd.user_id
        LEFT JOIN LATERAL (
            SELECT
                COUNT(*)::bigint AS request_count,
                COALESCE(SUM(COALESCE(r.input_tokens, 0) + COALESCE(r.output_tokens, 0)), 0)::bigint
                    AS total_tokens,
                COALESCE(SUM(r.cost_microdollars), 0)::bigint AS cost_microdollars
            FROM ai_requests r
            WHERE r.user_id = bd.user_id
              AND r.created_at >= bd.invoked_at AND r.created_at < bd.window_end
        ) a ON TRUE
        ORDER BY bd.invoked_at DESC
        LIMIT $4
        "#,
        filter.since,
        filter.user_filter(),
        ATTRIBUTION_PAD_MINUTES,
        filter.limit,
    )
    .fetch_all(pool)
    .await
}

pub async fn list_skill_totals(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<Vec<SkillTotalRow>, sqlx::Error> {
    let invocations = list_skill_invocations(pool, filter).await?;
    Ok(fold_skill_totals(&invocations))
}

pub fn fold_skill_totals(invocations: &[SkillInvocationRow]) -> Vec<SkillTotalRow> {
    let mut by_skill: BTreeMap<&str, (SkillTotalRow, Vec<&UserId>)> = BTreeMap::new();
    for inv in invocations {
        let entry = by_skill
            .entry(inv.skill.as_str())
            .or_insert_with(|| (empty_total(&inv.skill), Vec::new()));
        let (total, users) = entry;
        total.invocation_count += 1;
        total.request_count += inv.request_count;
        total.total_tokens += inv.total_tokens;
        total.cost_microdollars += inv.cost_microdollars;
        total.first_used_at = Some(min_opt(total.first_used_at, inv.invoked_at));
        total.last_used_at = Some(max_opt(total.last_used_at, inv.invoked_at));
        if !users.contains(&&inv.user_id) {
            users.push(&inv.user_id);
        }
    }
    let mut out: Vec<SkillTotalRow> = by_skill
        .into_values()
        .map(|(mut total, users)| {
            total.distinct_users = users.len() as i64;
            total
        })
        .collect();
    out.sort_by(|a, b| {
        b.invocation_count
            .cmp(&a.invocation_count)
            .then_with(|| a.skill.cmp(&b.skill))
    });
    out
}

fn empty_total(skill: &str) -> SkillTotalRow {
    SkillTotalRow {
        skill: skill.to_owned(),
        invocation_count: 0,
        distinct_users: 0,
        request_count: 0,
        total_tokens: 0,
        cost_microdollars: 0,
        first_used_at: None,
        last_used_at: None,
    }
}

fn min_opt(current: Option<DateTime<Utc>>, candidate: DateTime<Utc>) -> DateTime<Utc> {
    current.map_or(candidate, |c| c.min(candidate))
}

fn max_opt(current: Option<DateTime<Utc>>, candidate: DateTime<Utc>) -> DateTime<Utc> {
    current.map_or(candidate, |c| c.max(candidate))
}
