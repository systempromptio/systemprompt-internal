//! Typed view-model structs for the site analytics dashboard
//! (`analytics-dashboard`). Mirrors every `{{field}}` / `{{#each}}` /
//! `{{#if}}` referenced by
//! `storage/files/admin/templates/analytics-dashboard.hbs`.

use serde::Serialize;
use serde::ser::SerializeMap;

use crate::handlers::ssr::types::{PieView, SvgLineChartView};

pub(super) use super::context_overview::{
    BucketLinkView, DashboardTabLink, DashboardTimeRange, FastSlowView, FiltersView, KpiStripView,
    LeaderRowView, LeaderboardView, ScopeChipView, SloOption, SortLinkView,
};
pub(super) use super::context_tabs::{
    ContainerRowView, CostTabView, ModelUsageRowView, ModelsTabView, SessionCostRowView,
    SessionsTabView, SkillRowView, SkillsTabView, SupplierRowView, ToolRowView, ToolServerRowView,
    ToolsTabView,
};

// Why: each tab is its own GET so it can be bookmarked, and so only the
// queries that tab renders ever run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum DashboardTab {
    Overview,
    Models,
    Skills,
    Tools,
    Sessions,
    Cost,
}

impl DashboardTab {
    // Why: anything unrecognised lands on Overview — a mistyped tab in a
    // shared link should still show the page rather than a 400.
    pub(super) fn from_query(raw: Option<&str>) -> Self {
        match raw {
            Some("models") => Self::Models,
            Some("skills") => Self::Skills,
            Some("tools") => Self::Tools,
            Some("sessions") => Self::Sessions,
            Some("cost") => Self::Cost,
            _ => Self::Overview,
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Models => "models",
            Self::Skills => "skills",
            Self::Tools => "tools",
            Self::Sessions => "sessions",
            Self::Cost => "cost",
        }
    }

    const ALL: [Self; 6] = [
        Self::Overview,
        Self::Models,
        Self::Skills,
        Self::Tools,
        Self::Sessions,
        Self::Cost,
    ];
}

// Why: The active tab, projected onto the `is_<tab>` flags the template
// branches on: one `true` and the rest `false`, flattened into the page.
#[derive(Debug, Clone, Copy)]
pub(super) struct ActiveDashboardTab(pub(super) DashboardTab);

impl Serialize for ActiveDashboardTab {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(DashboardTab::ALL.len()))?;
        for tab in DashboardTab::ALL {
            map.serialize_entry(&format!("is_{}", tab.as_str()), &(tab == self.0))?;
        }
        map.end()
    }
}

#[derive(Debug, Serialize)]
pub(super) struct AnalyticsDashboardContext {
    pub page: &'static str,
    pub title: String,
    pub time_range: DashboardTimeRange,
    pub tabs: Vec<DashboardTabLink>,
    // Why: the window's headline figures, on the toolbar rather than under the
    // title. The page header used to carry a meta line saying the same thing,
    // which cost a row of vertical space the tables needed.
    pub toolbar_count: String,
    pub breadcrumbs: Vec<Crumb>,
    #[serde(flatten)]
    pub active: ActiveDashboardTab,
    // Why: the one thing a reader must be told before summing a column. Member
    // attribution counts a person in every container they belong to, so the
    // rows deliberately overlap and the page says so rather than letting the
    // reader discover it by adding up to more than the instance.
    pub is_member_view: bool,
    pub attribution_links: Vec<AttributionLink>,

    pub filters: FiltersView,
    pub chips: Vec<ScopeChipView>,
    pub has_active_filters: bool,
    pub clear_url: String,
    pub base_url: &'static str,

    pub kpis: KpiStripView,
    pub volume_chart: SvgLineChartView,
    pub cost_chart: SvgLineChartView,
    pub model_pie: PieView,
    pub model_cost_chart: SvgLineChartView,

    pub leaderboard: LeaderboardView,

    pub slo_options: Vec<SloOption>,
    pub latency_link: String,
    pub fast_slow: FastSlowView,

    pub models: ModelsTabView,
    pub skills: SkillsTabView,
    pub tools: ToolsTabView,
    pub sessions: SessionsTabView,
    pub cost: CostTabView,
    pub export: crate::export::ExportView,
}

#[derive(Debug, Serialize)]
pub(super) struct Crumb {
    pub label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub href: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct AttributionLink {
    pub label: &'static str,
    pub href: String,
    pub is_active: bool,
    pub hint: &'static str,
}

#[derive(Debug, Default, Serialize)]
pub(super) struct KpiTile {
    pub label: String,
    pub value: String,
    pub note: String,
    pub tone: &'static str,
}
