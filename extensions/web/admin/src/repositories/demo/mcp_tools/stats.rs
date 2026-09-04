//! Per-tool rollup: call volume, failures, governance verdicts, approvals.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use sqlx::PgPool;

use super::invocations::list_mcp_tool_invocations;
use crate::repositories::demo::filter::DemoFilter;

#[derive(Debug, Clone, Default)]
pub struct McpToolStatRow {
    pub server: String,
    pub tool: String,
    pub call_count: i64,
    pub failure_count: i64,
    pub failure_rate: f64,
    pub distinct_users: i64,
    pub allowed: i64,
    pub denied: i64,
    pub held: i64,
    pub approved: i64,
    pub rejected: i64,
    pub approval_pending: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
    pub last_used_at: Option<DateTime<Utc>>,
}

pub async fn list_mcp_tool_stats(
    pool: &PgPool,
    filter: &DemoFilter,
) -> Result<Vec<McpToolStatRow>, sqlx::Error> {
    let mut stats = get_event_stats(pool, filter).await?;
    apply_governance(pool, filter, &mut stats).await?;
    apply_approvals(pool, filter, &mut stats).await?;
    apply_attributed_usage(pool, filter, &mut stats).await?;

    let mut out: Vec<McpToolStatRow> = stats.into_values().collect();
    for row in &mut out {
        row.failure_rate = if row.call_count > 0 {
            row.failure_count as f64 / row.call_count as f64
        } else {
            0.0
        };
    }
    out.sort_by(|a, b| {
        b.call_count
            .cmp(&a.call_count)
            .then_with(|| (&a.server, &a.tool).cmp(&(&b.server, &b.tool)))
    });
    Ok(out)
}

type StatMap = HashMap<(String, String), McpToolStatRow>;

async fn get_event_stats(pool: &PgPool, filter: &DemoFilter) -> Result<StatMap, sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            split_part(tool_name, '__', 2) AS "server!",
            substr(
                tool_name,
                length('mcp__' || split_part(tool_name, '__', 2) || '__') + 1
            ) AS "tool!",
            COUNT(*)::bigint AS "call_count!",
            COUNT(*) FILTER (WHERE event_type = 'PostToolUseFailure')::bigint AS "failure_count!",
            COUNT(DISTINCT user_id)::bigint AS "distinct_users!",
            MAX(created_at) AS "last_used_at?"
        FROM plugin_usage_events
        WHERE created_at >= $1
          AND ($2::text IS NULL OR user_id = $2)
          AND tool_name LIKE 'mcp\_\_%'
          AND event_type IN ('PostToolUse', 'PostToolUseFailure')
        GROUP BY 1, 2
        "#,
        filter.since,
        filter.user_filter(),
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| {
            let key = (r.server.clone(), r.tool.clone());
            let stat = McpToolStatRow {
                server: r.server,
                tool: r.tool,
                call_count: r.call_count,
                failure_count: r.failure_count,
                distinct_users: r.distinct_users,
                last_used_at: r.last_used_at,
                ..McpToolStatRow::default()
            };
            (key, stat)
        })
        .collect())
}

async fn apply_governance(
    pool: &PgPool,
    filter: &DemoFilter,
    stats: &mut StatMap,
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            CASE WHEN g.tool_name LIKE 'mcp\_\_%' THEN substr(g.tool_name, length('mcp__' || split_part(g.tool_name, '__', 2) || '__') + 1) ELSE g.tool_name END AS "tool!",
            COUNT(*) FILTER (WHERE g.decision = 'allow')::bigint   AS "allowed!",
            COUNT(*) FILTER (WHERE g.decision = 'deny')::bigint    AS "denied!",
            COUNT(*) FILTER (WHERE g.decision = 'pending')::bigint AS "held!"
        FROM governance_decisions g
        WHERE g.created_at >= $1
          AND ($2::text IS NULL OR g.user_id = $2)
          AND g.policy <> 'authz' AND NOT (g.policy = 'authz_rule_based' AND g.plugin_id IS NULL)
        GROUP BY 1
        "#,
        filter.since,
        filter.user_filter(),
    )
    .fetch_all(pool)
    .await?;

    for r in rows {
        for stat in stats.values_mut().filter(|s| s.tool == r.tool) {
            stat.allowed += r.allowed;
            stat.denied += r.denied;
            stat.held += r.held;
        }
    }
    Ok(())
}

async fn apply_approvals(
    pool: &PgPool,
    filter: &DemoFilter,
    stats: &mut StatMap,
) -> Result<(), sqlx::Error> {
    let rows = sqlx::query!(
        r#"
        SELECT
            server_name AS "server!",
            tool_name   AS "tool!",
            COUNT(*) FILTER (WHERE status = 'approved')::bigint AS "approved!",
            COUNT(*) FILTER (WHERE status = 'denied')::bigint   AS "rejected!",
            COUNT(*) FILTER (WHERE status = 'pending')::bigint  AS "pending!"
        FROM approval_requests
        WHERE created_at >= $1
          AND ($2::text IS NULL OR requested_by = $2)
        GROUP BY 1, 2
        "#,
        filter.since,
        filter.user_filter(),
    )
    .fetch_all(pool)
    .await?;

    for r in rows {
        let entry = stats
            .entry((r.server.clone(), r.tool.clone()))
            .or_insert_with(|| McpToolStatRow {
                server: r.server,
                tool: r.tool,
                ..McpToolStatRow::default()
            });
        entry.approved += r.approved;
        entry.rejected += r.rejected;
        entry.approval_pending += r.pending;
    }
    Ok(())
}

async fn apply_attributed_usage(
    pool: &PgPool,
    filter: &DemoFilter,
    stats: &mut StatMap,
) -> Result<(), sqlx::Error> {
    for inv in list_mcp_tool_invocations(pool, filter).await? {
        if let Some(stat) = stats.get_mut(&(inv.server, inv.tool)) {
            stat.total_tokens += inv.total_tokens;
            stat.cost_microdollars += inv.cost_microdollars;
        }
    }
    Ok(())
}
