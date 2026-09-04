//! Merged chronological demo logbook: skills, MCP calls, decisions, approvals.
//!
//! The per-request server authorization rows are dropped unconditionally by the
//! predicate documented in [`super::policy`]. `include_allows = false` then
//! drops the remaining `allow` verdicts, which are real but routine and would
//! bury what a demo is watching for: the refusals, the holds, and the tool
//! calls themselves.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

use super::filter::DemoFilter;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogbookKind {
    Skill,
    McpTool,
    Decision,
    Approval,
}

impl LogbookKind {
    fn from_tag(tag: &str) -> Self {
        match tag {
            "skill" => Self::Skill,
            "mcp_tool" => Self::McpTool,
            "approval" => Self::Approval,
            _ => Self::Decision,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LogbookRow {
    pub kind: LogbookKind,
    pub at: DateTime<Utc>,
    pub user_id: UserId,
    pub user_email: Option<String>,
    pub session_id: SessionId,
    pub label: String,
    pub detail: Option<String>,
    pub status: Option<String>,
    pub policy: Option<String>,
}

#[expect(
    clippy::too_many_lines,
    reason = "body is one irreducible compile-time-checked query! SQL literal: a four-branch UNION"
)]
pub async fn list_demo_logbook(
    pool: &PgPool,
    filter: &DemoFilter,
    include_allows: bool,
) -> Result<Vec<LogbookRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        WITH merged AS (
            SELECT
                'skill'::text                        AS kind,
                e.created_at                         AS at,
                e.user_id                            AS user_id,
                e.session_id                         AS session_id,
                e.metadata->'tool_input'->>'skill'   AS label,
                e.plugin_id                          AS detail,
                e.event_type                         AS status,
                NULL::text                           AS policy
            FROM plugin_usage_events e
            WHERE e.created_at >= $1
              AND ($2::text IS NULL OR e.user_id = $2)
              AND e.tool_name = 'Skill'
              AND e.metadata->'tool_input'->>'skill' IS NOT NULL

            UNION ALL

            SELECT
                'mcp_tool'::text,
                e.created_at,
                e.user_id,
                e.session_id,
                e.tool_name,
                split_part(e.tool_name, '__', 2),
                CASE WHEN e.event_type = 'PostToolUseFailure' THEN 'failure' ELSE 'ok' END,
                NULL::text
            FROM plugin_usage_events e
            WHERE e.created_at >= $1
              AND ($2::text IS NULL OR e.user_id = $2)
              AND e.tool_name LIKE 'mcp\_\_%'
              AND e.event_type IN ('PostToolUse', 'PostToolUseFailure')

            UNION ALL

            SELECT
                'decision'::text,
                g.created_at,
                g.user_id,
                g.session_id,
                g.tool_name,
                g.reason,
                g.decision,
                g.policy
            FROM governance_decisions g
            WHERE g.created_at >= $1
              AND ($2::text IS NULL OR g.user_id = $2)
              AND g.policy <> 'authz' AND NOT (g.policy = 'authz_rule_based' AND g.plugin_id IS NULL)
              AND (g.decision IN ('deny', 'pending') OR $3)

            UNION ALL

            SELECT
                'approval'::text,
                a.created_at,
                a.requested_by,
                COALESCE(a.session_id, ''),
                a.tool_name,
                a.server_name,
                a.status,
                'require_approval'::text
            FROM approval_requests a
            WHERE a.created_at >= $1
              AND ($2::text IS NULL OR a.requested_by = $2)
        )
        SELECT
            m.kind        AS "kind!",
            m.at          AS "at!",
            m.user_id     AS "user_id!: UserId",
            u.email       AS "user_email?",
            m.session_id  AS "session_id!: SessionId",
            m.label       AS "label!",
            m.detail      AS "detail?",
            m.status      AS "status?",
            m.policy      AS "policy?"
        FROM merged m
        LEFT JOIN users u ON u.id = m.user_id
        ORDER BY m.at DESC
        LIMIT $4
        "#,
        filter.since,
        filter.user_filter(),
        include_allows,
        filter.limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| LogbookRow {
            kind: LogbookKind::from_tag(&r.kind),
            at: r.at,
            user_id: r.user_id,
            user_email: r.user_email,
            session_id: r.session_id,
            label: r.label,
            detail: r.detail,
            status: r.status,
            policy: r.policy,
        })
        .collect())
}
