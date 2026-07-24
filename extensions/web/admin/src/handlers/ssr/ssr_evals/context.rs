//! Typed view-model structs for the Evals page. Mirrors every `{{field}}`,
//! `{{#each}}`, and `{{#if}}` referenced by
//! `storage/files/admin/templates/evals.hbs` and `eval-run-detail.hbs`.

use serde::Serialize;
use systemprompt::identifiers::UserId;

#[derive(Debug, Serialize)]
pub(super) struct EvalLatencyBucketView {
    pub label: String,
    pub count: i64,
    pub pct: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct EvalCostBucketView {
    pub bucket_start: String,
    pub cost_microdollars: i64,
    pub pct: i64,
}

#[derive(Debug, Serialize)]
pub(super) struct EvalsPageContext {
    pub page: &'static str,
    pub title: &'static str,
    pub time_range: EvalTimeRangeView,
    pub traffic: TrafficStatsView,
    pub scores: ScoreSummaryView,
    pub histogram: Vec<EvalLatencyBucketView>,
    pub histogram_max: i64,
    pub cost_series: Vec<EvalCostBucketView>,
    pub cost_max: i64,
    pub models: Vec<ModelMixRowView>,
    pub users: Vec<UserRowView>,
    pub topics: Vec<TopicRowView>,
    pub win_rates: Vec<WinRateView>,
    pub runs: Vec<RunRowView>,
    pub results: Vec<ResultRowView>,
    pub cases: Vec<CaseRowView>,
    pub model_options: Vec<ModelOptionView>,
    pub judge_model: String,
    pub default_sample_size: i64,
    pub max_sample_size: i64,
    pub base_url: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notice: Option<NoticeView>,
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
