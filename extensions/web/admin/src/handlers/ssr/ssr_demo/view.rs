//! Row view-models shared by the four demo pages.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::handlers::ssr::format::format_token_total;
use crate::repositories::demo::logbook::{LogbookKind, LogbookRow};
use crate::repositories::demo::mcp_tools::McpToolStatRow;
use crate::repositories::demo::skill_invocations::SkillTotalRow;
use crate::types::SkillCatalogEntry;
use systemprompt::identifiers::SessionId;

pub(super) use super::matrix_view::{MatrixView, UserTotalView, matrix_view, user_total_views};

#[derive(Debug, Serialize)]
// Why: `testid` and `variant` are always serialized, empty when unset — the
// stat-card partial receives them as hash parameters, and a skipped field is a
// missing-path lookup under the engine's strict mode.
pub(super) struct KpiView {
    pub label: &'static str,
    pub value: String,
    pub testid: &'static str,
    pub variant: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct ScenarioCard {
    pub heading: &'static str,
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
    pub session_id: SessionId,
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
    // Why: which signal produced these invocations. "slash command" is the
    // user typing /plugin:skill; "tool call" is the model dispatching the
    // Skill tool. A skill that has done both reads "both".
    pub source_label: &'static str,
    // Why: true when the recorded name matches no skill in
    // services/skills. The row was real when it was written, so it is marked
    // rather than hidden.
    pub is_retired: bool,
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

#[derive(Debug, Default, Clone, Copy)]
pub(super) struct AttributedTotals {
    pub total_tokens: i64,
    pub cost_microdollars: i64,
}

impl AttributedTotals {
    pub(super) const fn add(&mut self, tokens: i64, cost_microdollars: i64) {
        self.total_tokens += tokens;
        self.cost_microdollars += cost_microdollars;
    }
}

#[derive(Debug, Default)]
pub(super) struct ToolVerdictTotals {
    pub allowed: i64,
    pub denied: i64,
    pub held: i64,
    pub approved: i64,
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

const MICRODOLLARS_PER_CENT: i64 = 10_000;

// Why: attributed cost is often a fraction of a cent, and printing six decimals
// reads as noise rather than as a small number. One rule, used by every demo
// page: cents and above get two decimals, anything non-zero below a cent is
// reported as under a cent rather than rounded away to zero.
pub(super) fn format_demo_cost(microdollars: i64) -> String {
    if microdollars <= 0 {
        return "$0.00".to_owned();
    }
    if microdollars < MICRODOLLARS_PER_CENT {
        return "<$0.01".to_owned();
    }
    format!("${:.2}", microdollars as f64 / 1_000_000.0)
}

pub(super) fn describe_user(email: Option<&String>, fallback: &str) -> String {
    email.map_or_else(|| fallback.to_owned(), Clone::clone)
}

const fn kind_labels(kind: LogbookKind) -> (&'static str, &'static str) {
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
        session_id: row.session_id.clone(),
        label: row.label.clone(),
        detail: row.detail.clone().unwrap_or_default(),
        status_color: status_color(&status),
        status,
        policy: row.policy.clone().unwrap_or_default(),
    }
}

// Why: skills are recorded as `plugin:skill`; splitting here keeps the table
// sortable by plugin without a second query.
// Why: the demo tables key everything on a `qualifier:name` string — a skill is
// `plugin:skill`, an MCP tool is `server:tool`. One split, so a matrix column
// header and a table row can never disagree about where the boundary is.
pub(super) fn split_qualified(id: &str) -> (String, String) {
    id.split_once(':').map_or_else(
        || (String::new(), id.to_owned()),
        |(qualifier, name)| (qualifier.to_owned(), name.to_owned()),
    )
}

// Why: a skill id is stored hyphenated because `plugin_resolvers.rs` builds
// the slash command as `/{plugin}:{skill_id.replace('_', "-")}`, while
// services/skills ids keep their underscores. Compare on the hyphenated form
// so the two namespaces meet.
#[derive(Debug, Default)]
pub(super) struct SkillCatalogIndex {
    plugins: BTreeSet<String>,
    skills: BTreeSet<String>,
}

impl SkillCatalogIndex {
    pub(super) fn new(plugins: &[String], skills: &[SkillCatalogEntry]) -> Self {
        Self {
            plugins: plugins.iter().cloned().collect(),
            skills: skills
                .iter()
                .map(|s| s.id.as_str().replace('_', "-"))
                .collect(),
        }
    }

    // Why: only a plugin this instance actually ships can be judged. Rows from
    // a marketplace plugin the operator installed elsewhere -- systemprompt-crm,
    // systemprompt-commons -- have no local definition to be absent from, so
    // calling them retired would mislabel most of the table.
    fn is_retired(&self, plugin: &str, skill: &str) -> bool {
        self.plugins.contains(plugin) && !self.skills.contains(skill)
    }
}

// Why: read the on-disk catalog so a recorded name absent from services/skills
// is marked retired instead of passing as live. A failure here costs only the
// badge, so it degrades to "nothing is judged".
pub(super) fn load_skill_catalog() -> SkillCatalogIndex {
    crate::handlers::shared::get_services_path()
        .ok()
        .map_or_else(SkillCatalogIndex::default, |path| {
            let plugins = crate::repositories::marketplace::plugins::list_plugin_catalog(&path)
                .unwrap_or_default()
                .into_iter()
                .map(|p| p.id)
                .collect::<Vec<_>>();
            let skills = crate::repositories::marketplace::plugins::list_skill_catalog(&path)
                .unwrap_or_default();
            SkillCatalogIndex::new(&plugins, &skills)
        })
}

const fn source_label(slash: i64, tool: i64) -> &'static str {
    match (slash > 0, tool > 0) {
        (true, true) => "both",
        (false, true) => "tool call",
        _ => "slash command",
    }
}

pub(super) fn skill_total_view(row: &SkillTotalRow, catalog: &SkillCatalogIndex) -> SkillTotalView {
    let (plugin, skill) = split_qualified(&row.skill);
    let is_retired = catalog.is_retired(&plugin, &skill);
    SkillTotalView {
        source_label: source_label(row.slash_count, row.tool_count),
        is_retired,
        plugin,
        skill,
        invocation_count: row.invocation_count,
        distinct_users: row.distinct_users,
        request_count: row.request_count,
        tokens_display: format_token_total(row.total_tokens),
        cost_display: format_demo_cost(row.cost_microdollars),
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
        cost_display: format_demo_cost(row.cost_microdollars),
        last_used_at: row.last_used_at.map(|d| d.to_rfc3339()),
    }
}
