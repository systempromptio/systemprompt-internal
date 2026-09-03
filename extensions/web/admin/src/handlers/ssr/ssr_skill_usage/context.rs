//! Typed template context for `skill-usage.hbs`.

use serde::Serialize;

#[derive(Debug, Serialize)]
pub(super) struct SkillsPageContext {
    pub(super) page: &'static str,
    pub(super) title: &'static str,
    pub(super) skills: Vec<SkillRowView>,
    pub(super) page_stats: Vec<PageStat>,
}

#[derive(Debug, Serialize)]
pub(super) struct SkillRowView {
    pub(super) skill_id: String,
    pub(super) invocation_count: i64,
    pub(super) distinct_users: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) first_used_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) last_used_at: Option<String>,
    pub(super) estimated_session_count: i64,
    pub(super) estimated_request_count: i64,
    pub(super) estimated_tokens: i64,
    pub(super) estimated_cost_usd: f64,
}

#[derive(Debug, Serialize)]
pub(super) struct PageStat {
    pub(super) value: i64,
    pub(super) label: &'static str,
}
