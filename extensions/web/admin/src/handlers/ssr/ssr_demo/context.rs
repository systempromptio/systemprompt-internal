//! Typed template contexts for the four demo pages.

use serde::Serialize;

use super::view::{
    KpiView, LogbookRowView, MatrixView, McpToolStatView, ScenarioCard, ServerCardView,
    SkillTotalView, UserTotalView,
};
use crate::handlers::ssr::types::ChartView;

#[derive(Debug, Serialize)]
pub(super) struct DemoLogbookContext {
    pub page: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub kpis: Vec<KpiView>,
    pub scenarios: Vec<ScenarioCard>,
    pub rows: Vec<LogbookRowView>,
    pub has_rows: bool,
    pub attribution_note: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct DemoSkillsContext {
    pub page: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub kpis: Vec<KpiView>,
    pub chart: ChartView,
    pub skills: Vec<SkillTotalView>,
    pub has_skills: bool,
    pub matrix: MatrixView,
    pub user_totals: Vec<UserTotalView>,
    pub has_user_totals: bool,
    pub attribution_note: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct DemoToolsContext {
    pub page: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub kpis: Vec<KpiView>,
    pub chart: ChartView,
    pub servers: Vec<ServerCardView>,
    pub has_servers: bool,
    pub tools: Vec<McpToolStatView>,
    pub has_tools: bool,
    pub matrix: MatrixView,
    pub attribution_note: &'static str,
}

#[derive(Debug, Serialize)]
pub(super) struct DemoMeContext {
    pub page: &'static str,
    pub title: &'static str,
    pub subtitle: &'static str,
    pub user_email: String,
    pub kpis: Vec<KpiView>,
    pub chart: ChartView,
    pub tool_chart: ChartView,
    pub skills: Vec<SkillTotalView>,
    pub has_skills: bool,
    pub tools: Vec<McpToolStatView>,
    pub has_tools: bool,
    pub rows: Vec<LogbookRowView>,
    pub has_rows: bool,
    pub attribution_note: &'static str,
}
