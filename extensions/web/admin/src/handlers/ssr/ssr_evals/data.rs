//! Data collection for the Evals page.
//!
//! Same shape as the Inference Requests page: resolve the window (auto-widen
//! until it has rows), then fan every repository call out in parallel and
//! collapse each `Result` into a logged default, so one failing query degrades
//! a panel instead of the page.

use std::sync::Arc;

use sqlx::PgPool;

use crate::repositories::analytics::request_stats::{
    CostBucket, LatencyBucket, RequestStats, get_request_stats, list_cost_over_time,
    list_latency_histogram,
};
use crate::repositories::evals::cases::{EvalCaseRow, list_cases};
use crate::repositories::evals::distribution::{
    EvalScoreSummary, ModelDistributionRow, ModelScoreRow, ModelWinRateRow, PromptTopicRow,
    UserDistributionRow, get_eval_score_summary, list_model_distribution, list_model_scores,
    list_model_win_rates, list_prompt_topics, list_user_distribution,
};
use crate::repositories::evals::results::{EvalResultRow, list_recent_results};
use crate::repositories::evals::runs::{EvalRunRow, list_recent_runs};
use crate::util::time_range::{
    TimeRange, TimeRangePreset, TimeRangeQuery, count_requests_in_range, parse_time_range,
    preset_to_range,
};

use super::EvalsQuery;

/// How many rows each panel shows before it stops being a summary.
const USER_LIMIT: i64 = 15;
const TOPIC_LIMIT: i64 = 15;
const RUN_LIMIT: i64 = 15;
const RESULT_LIMIT: i64 = 50;

pub(super) async fn resolve_range(
    pool: &PgPool,
    query: &EvalsQuery,
) -> (TimeRange, Option<&'static str>) {
    let user_picked_range = query.preset.is_some() || (query.from.is_some() && query.to.is_some());
    let initial_range = parse_time_range(&TimeRangeQuery {
        from: query.from.clone(),
        to: query.to.clone(),
        preset: query.preset.clone(),
    });

    if user_picked_range {
        return (initial_range, None);
    }

    let mut chosen = initial_range;
    let mut widened: Option<&'static str> = None;
    for (label, preset) in [
        ("24h", TimeRangePreset::Hours24),
        ("7d", TimeRangePreset::Days7),
        ("30d", TimeRangePreset::Days30),
    ] {
        let candidate = preset_to_range(preset);
        let count = count_requests_in_range(pool, candidate).await.unwrap_or(0);
        if count > 0 {
            chosen = candidate;
            widened = if label == "24h" { None } else { Some(label) };
            break;
        }
    }
    (chosen, widened)
}

pub(super) fn range_from_strings(from: Option<&str>, to: Option<&str>) -> TimeRange {
    parse_time_range(&TimeRangeQuery {
        from: from.map(str::to_owned),
        to: to.map(str::to_owned),
        preset: None,
    })
}

pub(super) struct EvalsData {
    pub stats: RequestStats,
    pub hist: Vec<LatencyBucket>,
    pub cost: Vec<CostBucket>,
    pub models: Vec<ModelDistributionRow>,
    pub model_scores: Vec<ModelScoreRow>,
    pub users: Vec<UserDistributionRow>,
    pub topics: Vec<PromptTopicRow>,
    pub scores: EvalScoreSummary,
    pub win_rates: Vec<ModelWinRateRow>,
    pub runs: Vec<EvalRunRow>,
    pub results: Vec<EvalResultRow>,
    pub cases: Vec<EvalCaseRow>,
}

pub(super) async fn fetch_evals_data(pool: &Arc<PgPool>, range: TimeRange) -> EvalsData {
    let (stats, hist, cost, models, model_scores, users) = tokio::join!(
        get_request_stats(pool, range),
        list_latency_histogram(pool, range),
        list_cost_over_time(pool, range),
        list_model_distribution(pool, range),
        list_model_scores(pool, range),
        list_user_distribution(pool, range, USER_LIMIT),
    );
    let (topics, scores, win_rates, runs, results, cases) = tokio::join!(
        list_prompt_topics(pool, range, TOPIC_LIMIT),
        get_eval_score_summary(pool, range),
        list_model_win_rates(pool, range),
        list_recent_runs(pool, range, RUN_LIMIT),
        list_recent_results(pool, range, RESULT_LIMIT),
        list_cases(pool, false),
    );

    EvalsData {
        stats: unwrap_or_default(stats, "get_request_stats"),
        hist: unwrap_or_empty(hist, "list_latency_histogram"),
        cost: unwrap_or_empty(cost, "list_cost_over_time"),
        models: unwrap_or_empty(models, "list_model_distribution"),
        model_scores: unwrap_or_empty(model_scores, "list_model_scores"),
        users: unwrap_or_empty(users, "list_user_distribution"),
        topics: unwrap_or_empty(topics, "list_prompt_topics"),
        scores: unwrap_or_default(scores, "get_eval_score_summary"),
        win_rates: unwrap_or_empty(win_rates, "list_model_win_rates"),
        runs: unwrap_or_empty(runs, "list_recent_runs"),
        results: unwrap_or_empty(results, "list_recent_results"),
        cases: unwrap_or_empty(cases, "list_cases"),
    }
}

fn unwrap_or_empty<T>(res: Result<Vec<T>, sqlx::Error>, what: &str) -> Vec<T> {
    res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, query = what, "evals page query failed");
        Vec::new()
    })
}

fn unwrap_or_default<T: Default>(res: Result<T, sqlx::Error>, what: &str) -> T {
    res.unwrap_or_else(|e| {
        tracing::warn!(error = %e, query = what, "evals page query failed");
        T::default()
    })
}
