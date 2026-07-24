//! View-model assembly for the Evals page.
//!
//! Pure functions from repository rows to the serde JSON the templates
//! consume. Formatting decisions live here and nowhere else: the templates do
//! no arithmetic, so what a reader sees on the page is exactly what a reader
//! of this file computes.

use std::collections::HashMap;

use urlencoding::encode as urlencode;

use crate::repositories::analytics::request_stats::{CostBucket, LatencyBucket, RequestStats};
use crate::repositories::evals::distribution::{
    EvalScoreSummary, ModelDistributionRow, ModelScoreRow, ModelWinRateRow, PromptTopicRow,
    UserDistributionRow,
};
use crate::util::time_range::TimeRange;

use super::format::{bar_pct, format_cost, local_time, score_pct, share_pct, truncate};

use super::context::{
    EvalCostBucketView, EvalLatencyBucketView, ModelOptionView, ModelMixRowView,
    ScoreSummaryView, EvalTimeRangeView, TopicRowView, TrafficStatsView,
    UserRowView, WinRateView,
};
use super::{BASE_URL, EvalsQuery};

pub(super) fn traffic_stats(
    s: &RequestStats,
    models: &[ModelDistributionRow],
    users: &[UserDistributionRow],
) -> TrafficStatsView {
    TrafficStatsView {
        total: s.total,
        error_count: s.error_count,
        error_rate_pct: format!("{:.2}", s.error_rate * 100.0),
        p50_latency_ms: s.p50_latency_ms.round() as i64,
        p95_latency_ms: s.p95_latency_ms.round() as i64,
        total_cost_display: format_cost(s.total_cost_microdollars),
        user_count: users.len() as i64,
        model_count: models.len() as i64,
    }
}

pub(super) fn score_summary(s: &EvalScoreSummary, traffic_total: i64) -> ScoreSummaryView {
    let coverage = if traffic_total > 0 {
        s.scored_count as f64 / traffic_total as f64 * 100.0
    } else {
        0.0
    };
    ScoreSummaryView {
        scored_count: s.scored_count,
        mean_score_display: if s.scored_count > 0 {
            format!("{:.2}", s.mean_score)
        } else {
            "—".to_owned()
        },
        mean_score_pct: score_pct(s.mean_score),
        pass_count: s.pass_count,
        partial_count: s.partial_count,
        fail_count: s.fail_count,
        flagged_count: s.flagged_count,
        judge_cost_display: format_cost(s.judge_cost_microdollars),
        coverage_pct: format!("{coverage:.1}"),
        has_scores: s.scored_count > 0,
    }
}


pub(super) fn latency_buckets(hist: &[LatencyBucket]) -> (Vec<EvalLatencyBucketView>, i64) {
    let max = hist.iter().map(|b| b.count).max().unwrap_or(0);
    let views = hist
        .iter()
        .map(|b| EvalLatencyBucketView {
            label: b.label.clone(),
            count: b.count,
            pct: bar_pct(b.count, max),
        })
        .collect();
    (views, max)
}

pub(super) fn cost_buckets(cost: &[CostBucket]) -> (Vec<EvalCostBucketView>, i64) {
    let max = cost.iter().map(|b| b.cost_microdollars).max().unwrap_or(0);
    let views = cost
        .iter()
        .map(|b| EvalCostBucketView {
            bucket_start: b.bucket_start.to_rfc3339(),
            cost_microdollars: b.cost_microdollars,
            pct: bar_pct(b.cost_microdollars, max),
        })
        .collect();
    (views, max)
}

pub(super) fn model_rows(
    models: &[ModelDistributionRow],
    scores: &[ModelScoreRow],
    total_requests: i64,
) -> Vec<ModelMixRowView> {
    let by_model: HashMap<&str, &ModelScoreRow> =
        scores.iter().map(|s| (s.model.as_str(), s)).collect();

    models
        .iter()
        .map(|m| {
            let score = by_model.get(m.model.as_str());
            let cost_per_request = if m.request_count > 0 {
                m.cost_microdollars / m.request_count
            } else {
                0
            };
            ModelMixRowView {
                provider: m.provider.clone(),
                model: m.model.clone(),
                request_count: m.request_count,
                share_pct: share_pct(m.request_count, total_requests),
                user_count: m.user_count,
                error_count: m.error_count,
                tokens_total: m.input_tokens + m.output_tokens,
                cost_display: format_cost(m.cost_microdollars),
                cost_per_request_display: format_cost(cost_per_request),
                p50_latency_ms: m.p50_latency_ms.round() as i64,
                p95_latency_ms: m.p95_latency_ms.round() as i64,
                scored_count: score.map_or(0, |s| s.scored_count),
                mean_score_display: score
                    .filter(|s| s.scored_count > 0)
                    .map_or_else(|| "—".to_owned(), |s| format!("{:.2}", s.mean_score)),
                fail_count: score.map_or(0, |s| s.fail_count),
                has_score: score.is_some_and(|s| s.scored_count > 0),
            }
        })
        .collect()
}

pub(super) fn user_rows(users: &[UserDistributionRow], total_requests: i64) -> Vec<UserRowView> {
    users
        .iter()
        .map(|u| UserRowView {
            user_id: u.user_id.clone(),
            user_label: u
                .user_label
                .clone()
                .unwrap_or_else(|| u.user_id.as_str().to_owned()),
            request_count: u.request_count,
            share_pct: share_pct(u.request_count, total_requests),
            session_count: u.session_count,
            model_count: u.model_count,
            error_count: u.error_count,
            cost_display: format_cost(u.cost_microdollars),
            last_seen_local: local_time(u.last_seen),
        })
        .collect()
}

pub(super) fn topic_rows(topics: &[PromptTopicRow], total_requests: i64) -> Vec<TopicRowView> {
    topics
        .iter()
        .map(|t| TopicRowView {
            topic: t.topic.clone(),
            sample_excerpt: truncate(&t.sample_excerpt, 160),
            request_count: t.request_count,
            share_pct: share_pct(t.request_count, total_requests),
            distinct_models: t.distinct_models,
            cost_display: format_cost(t.cost_microdollars),
        })
        .collect()
}

pub(super) fn win_rate_rows(rows: &[ModelWinRateRow]) -> Vec<WinRateView> {
    rows.iter()
        .map(|r| {
            let decisive = r.wins + r.losses;
            WinRateView {
                model: r.model.clone(),
                comparisons: r.comparisons,
                wins: r.wins,
                losses: r.losses,
                ties: r.ties,
                win_rate_pct: if decisive > 0 {
                    ((r.wins as f64 / decisive as f64) * 100.0).round() as i64
                } else {
                    0
                },
            }
        })
        .collect()
}


pub(super) fn model_options(models: &[ModelDistributionRow]) -> Vec<ModelOptionView> {
    models
        .iter()
        .map(|m| ModelOptionView {
            value: format!("{}/{}", m.provider, m.model),
            label: format!("{} ({})", m.model, m.provider),
        })
        .collect()
}

pub(super) fn time_range_context(
    query: &EvalsQuery,
    range: &TimeRange,
    auto_widened: Option<&'static str>,
) -> EvalTimeRangeView {
    let preset = query.preset.clone().unwrap_or_else(|| {
        if query.from.is_some() && query.to.is_some() {
            "custom".to_owned()
        } else {
            auto_widened.unwrap_or("24h").to_owned()
        }
    });
    EvalTimeRangeView {
        preset,
        from: range.from.to_rfc3339(),
        to: range.to.to_rfc3339(),
        base_url: BASE_URL,
        query: String::new(),
        auto_widened,
    }
}

pub(super) fn redirect_url(range: &TimeRange, notice: &str, is_error: bool) -> String {
    format!(
        "{BASE_URL}?from={}&to={}&notice={}&notice_error={}",
        urlencode(&range.from.to_rfc3339()),
        urlencode(&range.to.to_rfc3339()),
        urlencode(notice),
        if is_error { "1" } else { "0" },
    )
}

