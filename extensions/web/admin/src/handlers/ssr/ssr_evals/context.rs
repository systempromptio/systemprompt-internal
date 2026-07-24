//! Typed view-model structs for the Evals page. Mirrors every `{{field}}`,
//! `{{#each}}`, and `{{#if}}` referenced by
//! `storage/files/admin/templates/evals.hbs` and `eval-run-detail.hbs`.

use serde::Serialize;

use crate::handlers::ssr::types::{ChartView, HistogramView};
use systemprompt::identifiers::UserId;

/// Which section of the page is being looked at. The page is split by *kind of
/// eval* rather than by data source, so the form that launches a run and the
/// table that shows its output live on the same tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EvalsTab {
    /// Health of the window: KPIs, charts, and every run regardless of kind.
    Overview,
    /// What actually went through the gateway, by model, user, and prompt shape.
    Traffic,
    /// Judge runs over live traffic, and the answers they scored.
    Judge,
    /// Pairwise runs: win rates and the individual comparisons behind them.
    HeadToHead,
    /// The golden set, and the replay runs that exercise it.
    GoldenSet,
}

impl EvalsTab {
    /// Anything unrecognised lands on Overview. A mistyped tab in a shared link
    /// should still show the page rather than a 400.
    pub(super) fn from_query(raw: Option<&str>) -> Self {
        match raw {
            Some("traffic") => Self::Traffic,
            Some("judge") => Self::Judge,
            Some("head-to-head") => Self::HeadToHead,
            Some("golden-set") => Self::GoldenSet,
            _ => Self::Overview,
        }
    }

    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Traffic => "traffic",
            Self::Judge => "judge",
            Self::HeadToHead => "head-to-head",
            Self::GoldenSet => "golden-set",
        }
    }
}

#[derive(Debug, Serialize)]
pub(super) struct EvalsPageContext {
    pub page: &'static str,
    pub title: &'static str,
    pub tab: &'static str,
    pub is_overview: bool,
    pub is_traffic: bool,
    pub is_judge: bool,
    pub is_head_to_head: bool,
    pub is_golden_set: bool,
    /// True on the tabs whose KPI strip is about traffic, not judged quality.
    pub show_traffic_kpis: bool,
    pub show_quality_kpis: bool,
    pub tabs: Vec<TabLinkView>,
    pub time_range: EvalTimeRangeView,
    pub traffic: TrafficStatsView,
    pub scores: ScoreSummaryView,
    pub histogram: HistogramView,
    pub cost_chart: ChartView,
    pub models: Vec<ModelMixRowView>,
    pub users: Vec<UserRowView>,
    pub topics: Vec<TopicRowView>,
    pub win_rates: Vec<WinRateView>,
    pub pairs: Vec<PairRowView>,
    pub runs: Vec<RunRowView>,
    pub results: Vec<ResultRowView>,
    pub cases: Vec<CaseRowView>,
    pub filter: ResultFilterView,
    pub model_options: Vec<ModelOptionView>,
    pub judge_model: String,
    pub default_sample_size: i64,
    pub max_sample_size: i64,
    pub base_url: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<NoticeView>,
}

#[derive(Debug, Serialize)]
pub(super) struct TabLinkView {
    pub slug: &'static str,
    pub label: &'static str,
    pub href: String,
    pub is_active: bool,
}

/// State of the Judge tab's verdict and model filters, echoed back so the
/// selects stay on what was picked after the round trip.
#[derive(Debug, Serialize)]
pub(super) struct ResultFilterView {
    pub verdict: String,
    pub model: String,
    pub is_filtered: bool,
    pub verdict_options: Vec<FilterOptionView>,
    pub model_options: Vec<FilterOptionView>,
}

#[derive(Debug, Serialize)]
pub(super) struct FilterOptionView {
    pub value: String,
    pub label: String,
    pub is_selected: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct PairRowView {
    pub model_a: String,
    pub model_b: String,
    pub winner_label: String,
    pub is_tie: bool,
    pub order_swapped: bool,
    pub rationale: String,
    pub created_at_local: String,
}

#[derive(Debug, Serialize)]
pub(super) struct NoticeView {
    pub is_error: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub(super) struct TrafficStatsView {
    pub total: i64,
    pub error_count: i64,
    pub error_rate_pct: String,
    pub p50_latency_ms: i64,
    pub p95_latency_ms: i64,
    pub total_cost_display: String,
    pub user_count: i64,
    pub model_count: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct ScoreSummaryView {
    pub scored_count: i64,
    pub mean_score_display: String,
    pub mean_score_pct: i64,
    pub pass_count: i64,
    pub partial_count: i64,
    pub fail_count: i64,
    pub flagged_count: i64,
    pub judge_cost_display: String,
    pub coverage_pct: String,
    pub has_scores: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct ModelMixRowView {
    pub provider: String,
    pub model: String,
    pub request_count: i64,
    pub share_pct: i64,
    pub user_count: i64,
    pub error_count: i64,
    pub tokens_total: i64,
    pub cost_display: String,
    pub cost_per_request_display: String,
    pub p50_latency_ms: i64,
    pub p95_latency_ms: i64,
    pub scored_count: i64,
    pub mean_score_display: String,
    pub fail_count: i64,
    pub has_score: bool,
}

#[derive(Debug, Serialize)]
pub(super) struct UserRowView {
    pub user_id: UserId,
    pub user_label: String,
    pub request_count: i64,
    pub share_pct: i64,
    pub session_count: i64,
    pub model_count: i64,
    pub error_count: i64,
    pub cost_display: String,
    pub last_seen_local: String,
}

#[derive(Debug, Serialize)]
pub(super) struct TopicRowView {
    pub topic: String,
    pub sample_excerpt: String,
    pub request_count: i64,
    pub share_pct: i64,
    pub distinct_models: i64,
    pub cost_display: String,
}

#[derive(Debug, Serialize)]
pub(super) struct WinRateView {
    pub model: String,
    pub comparisons: i64,
    pub wins: i64,
    pub losses: i64,
    pub ties: i64,
    pub win_rate_pct: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct RunRowView {
    pub id: String,
    pub short_id: String,
    pub kind: String,
    pub status: String,
    pub is_running: bool,
    pub is_failed: bool,
    pub judge_model: String,
    pub sample_size: i32,
    pub scored_count: i32,
    pub failed_count: i32,
    pub mean_score_display: String,
    pub cost_display: String,
    pub created_by: String,
    pub created_at_local: String,
    pub detail_url: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ResultRowView {
    pub id: String,
    pub run_id: String,
    /// Present for judged live traffic; absent for replayed golden-set cases.
    pub ai_request_id: Option<String>,
    pub case_id: Option<String>,
    pub model: String,
    pub provider: String,
    pub score_display: String,
    pub score_pct: i64,
    pub verdict: String,
    pub is_pass: bool,
    pub is_partial: bool,
    pub is_fail: bool,
    pub rationale: String,
    pub flags: Vec<String>,
    pub has_flags: bool,
    pub dimensions: Vec<DimensionView>,
    pub prompt_excerpt: String,
    pub response_excerpt: String,
    pub latency_ms: Option<i32>,
    pub created_at_local: String,
    pub promote_id: Option<String>,
}

#[derive(Debug, Serialize)]
pub(super) struct DimensionView {
    pub label: &'static str,
    pub score: i64,
    pub pct: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct CaseRowView {
    pub id: String,
    pub name: String,
    pub baseline_model: String,
    pub expectation: String,
    pub has_expectation: bool,
    pub created_at_local: String,
}

#[derive(Debug, Serialize)]
pub(super) struct ModelOptionView {
    /// `provider/model`, the form value the run POST parses back.
    pub value: String,
    pub label: String,
}

#[derive(Debug, Serialize)]
pub(super) struct EvalTimeRangeView {
    pub preset: String,
    pub from: String,
    pub to: String,
    pub base_url: &'static str,
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub auto_widened: Option<&'static str>,
}

#[derive(Debug, Serialize)]
pub(super) struct RunDetailContext {
    pub page: &'static str,
    pub title: String,
    pub run: RunRowView,
    pub results: Vec<ResultRowView>,
    pub back_url: &'static str,
}
