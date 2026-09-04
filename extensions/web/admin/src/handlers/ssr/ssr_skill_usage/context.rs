//! Typed template context for `skill-usage.hbs`.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct SkillsPageContext {
    pub(super) page: &'static str,
    pub(super) title: &'static str,
    pub(super) skills: Vec<SkillRowView>,
    pub(super) page_stats: Vec<PageStat>,
}

// Why: the same {value, label} shape as the contexts page's `PageStat`, kept
// separate because that one is shared verbatim with the template fork and this
// page is internal-only — merging them would drift a shared file to save four
// lines.
#[derive(Debug, Serialize)]
// lint-ok: duplicate-type — see above; the shared-fork copy must not move.
pub(super) struct PageStat {
    pub(super) value: i64,
    pub(super) label: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct SkillRowView {
    pub(super) skill: String,
    pub(super) invocation_count: i64,
    pub(super) distinct_users: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) first_used_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) last_used_at: Option<String>,
    pub(super) attributed_request_count: i64,
    pub(super) attributed_tokens: i64,
    pub(super) attributed_cost_usd: f64,
}
