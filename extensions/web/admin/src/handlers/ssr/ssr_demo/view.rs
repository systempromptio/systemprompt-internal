//! Row view-models shared by the four demo pages.

use serde::Serialize;

use crate::handlers::ssr::format::{format_cost, format_token_total};
use crate::repositories::demo::logbook::{LogbookKind, LogbookRow};
use crate::repositories::demo::mcp_tools::McpToolStatRow;
use crate::repositories::demo::skill_invocations::SkillTotalRow;
use crate::repositories::demo::{UsageMatrix, UsageMatrixRow};

#[derive(Debug, Serialize)]
pub(super) struct KpiView {
    pub label: &'static str,
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub testid: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variant: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub(super) struct ScenarioCard {
    pub letter: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub count: i64,
    pub href: &'static str,
    pub tone: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct LogbookRowView {
    pub kind: &'static str,
    pub kind_label: &'static str,
    pub at: String,
    pub user_email: String,
    pub session_id: String,
    pub label: String,
    pub detail: String,
    pub status: String,
    pub status_color: &'static str,
    pub policy: String,
}

#[derive(Debug, Serialize)]
pub(super) struct SkillTotalView {
    pub plugin: String,
    pub skill: String,
    pub invocation_count: i64,
    pub distinct_users: i64,
    pub request_count: i64,
    pub tokens_display: String,
    pub cost_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct McpToolStatView {
    pub server: String,
    pub tool: String,
    pub call_count: i64,
    pub failure_count: i64,
    pub failure_rate_display: String,
    pub distinct_users: i64,
    pub allowed: i64,
    pub denied: i64,
    pub held: i64,
    pub approved: i64,
    pub rejected: i64,
    pub approval_pending: i64,
    pub tokens_display: String,
    pub cost_display: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_used_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct ServerCardView {
    pub server: String,
    pub tool_count: i64,
    pub call_count: i64,
    pub failure_count: i64,
    pub denied: i64,
    pub held: i64,
    pub cost_display: String,
}

#[derive(Debug, Serialize)]
pub(super) struct UserTotalView {
    pub user_email: String,
    pub total: i64,
    pub tokens_display: String,
    pub cost_display: String,
}

#[derive(Debug, Serialize)]
pub(super) struct MatrixCellView {
    pub count: i64,
    pub pct: i64,
    pub is_zero: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct MatrixRowView {
    pub user_email: String,
    pub cells: Vec<MatrixCellView>,
    pub total: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct MatrixView {
    pub columns: Vec<String>,
    pub rows: Vec<MatrixRowView>,
    pub has_data: bool,
}

fn describe_user(email: Option<&String>, fallback: &str) -> String {
    email.map_or_else(|| fallback.to_owned(), Clone::clone)
}

fn kind_labels(kind: LogbookKind) -> (&'static str, &'static str) {
    match kind {
        LogbookKind::Skill => ("skill", "Skill"),
        LogbookKind::McpTool => ("mcp-tool", "MCP tool"),
        LogbookKind::Decision => ("decision", "Decision"),
        LogbookKind::Approval => ("approval", "Approval"),
    }
}

// Why: the logbook merges four sources whose status vocabularies differ, so the
// badge colour is decided once here rather than by a chain of template `#if`s.
fn status_color(status: &str) -> &'static str {
    match status {
        "allow" | "allowed" | "approved" | "success" => "green",
        "deny" | "denied" | "rejected" | "failure" => "red",
        "pending" | "ask" | "held" => "warning",
        _ => "gray",
    }
}

pub(super) fn logbook_row_view(row: &LogbookRow) -> LogbookRowView {
    let (kind, kind_label) = kind_labels(row.kind);
    let status = row.status.clone().unwrap_or_default();
    LogbookRowView {
        kind,
        kind_label,
        at: row.at.to_rfc3339(),
        user_email: describe_user(row.user_email.as_ref(), row.user_id.as_str()),
        session_id: row.session_id.as_str().to_owned(),
        label: row.label.clone(),
        detail: row.detail.clone().unwrap_or_default(),
        status_color: status_color(&status),
        status,
        policy: row.policy.clone().unwrap_or_default(),
    }
}

// Why: skills are recorded as `plugin:skill`; splitting here keeps the table
// sortable by plugin without a second query.
pub(super) fn skill_total_view(row: &SkillTotalRow) -> SkillTotalView {
    let (plugin, skill) = row
        .skill
        .split_once(':')
        .map_or((String::new(), row.skill.clone()), |(p, s)| {
            (p.to_owned(), s.to_owned())
        });
    SkillTotalView {
        plugin,
        skill,
        invocation_count: row.invocation_count,
        distinct_users: row.distinct_users,
        request_count: row.request_count,
        tokens_display: format_token_total(row.total_tokens),
        cost_display: format_cost(row.cost_microdollars),
        last_used_at: row.last_used_at.map(|d| d.to_rfc3339()),
    }
}

pub(super) fn mcp_tool_stat_view(row: &McpToolStatRow) -> McpToolStatView {
    McpToolStatView {
        server: row.server.clone(),
        tool: row.tool.clone(),
        call_count: row.call_count,
        failure_count: row.failure_count,
        failure_rate_display: format!("{:.1}%", row.failure_rate * 100.0),
        distinct_users: row.distinct_users,
        allowed: row.allowed,
        denied: row.denied,
        held: row.held,
        approved: row.approved,
        rejected: row.rejected,
        approval_pending: row.approval_pending,
        tokens_display: format_token_total(row.total_tokens),
        cost_display: format_cost(row.cost_microdollars),
        last_used_at: row.last_used_at.map(|d| d.to_rfc3339()),
    }
}

pub(super) fn matrix_view(matrix: &UsageMatrix) -> MatrixView {
    let max = matrix
        .rows
        .iter()
        .flat_map(|r| r.cells.iter().copied())
        .max()
        .unwrap_or(0);
    MatrixView {
        columns: matrix.columns.clone(),
        rows: matrix
            .rows
            .iter()
            .map(|r| matrix_row_view(r, max))
            .collect(),
        has_data: !matrix.columns.is_empty() && !matrix.rows.is_empty(),
    }
}

fn matrix_row_view(row: &UsageMatrixRow, max: i64) -> MatrixRowView {
    MatrixRowView {
        user_email: describe_user(row.user_email.as_ref(), row.user_id.as_str()),
        cells: row
            .cells
            .iter()
            .map(|&count| MatrixCellView {
                count,
                pct: crate::handlers::ssr::types::bar_pct(count, max),
                is_zero: count == 0,
            })
            .collect(),
        total: row.total,
    }
}

pub(super) fn user_total_views(matrix: &UsageMatrix) -> Vec<UserTotalView> {
    matrix
        .rows
        .iter()
        .map(|r| UserTotalView {
            user_email: describe_user(r.user_email.as_ref(), r.user_id.as_str()),
            total: r.total,
            tokens_display: format_token_total(r.total_tokens),
            cost_display: format_cost(r.cost_microdollars),
        })
        .collect()
}
