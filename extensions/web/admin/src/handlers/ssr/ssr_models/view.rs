//! Serializable view models for the `models.hbs` template.

use serde::Serialize;

use super::super::types::PageStatView;

#[derive(Debug, Serialize)]
pub(super) struct UserOptionView {
    pub id: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct ModelRowView {
    pub route_id: String,
    pub model_pattern: String,
    pub provider: String,
    pub upstream_model: String,
    pub denied: bool,
    pub deny_rule_id: String,
    pub status_label: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct UsageRowView {
    pub request_id: String,
    pub created_at: String,
    pub model: String,
    pub provider: String,
    pub status: String,
    pub is_completed: bool,
    pub tokens: String,
    pub cost: String,
    pub latency_ms: i64,
    pub deny_count: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct UsageTotalsView {
    pub requests: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost: String,
    pub denied_requests: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct ModelsPageData {
    pub page: &'static str,
    pub title: &'static str,
    pub users: Vec<UserOptionView>,
    pub has_selection: bool,
    pub selected_user_id: String,
    pub selected_user_label: String,
    pub models: Vec<ModelRowView>,
    pub usage: Vec<UsageRowView>,
    pub has_usage: bool,
    pub usage_totals: UsageTotalsView,
    pub requests_link: String,
    pub page_stats: Vec<PageStatView>,
}

pub(super) fn build_user_options(
    all_users: &[crate::types::UserSummary],
    selected_id: Option<&str>,
) -> Vec<UserOptionView> {
    all_users
        .iter()
        .map(|u| {
            let id = u.user_id.to_string();
            let label = u
                .email
                .as_ref()
                .map_or_else(|| id.clone(), |e| e.as_ref().to_owned());
            UserOptionView {
                selected: Some(id.as_str()) == selected_id,
                id,
                label,
            }
        })
        .collect()
}
