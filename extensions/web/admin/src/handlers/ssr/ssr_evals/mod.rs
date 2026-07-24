//! `/admin/evals` — traffic distribution and evaluation results.
//!
//! The page has two halves. The top half describes what actually went through
//! the gateway (models, users, prompt shapes, latency, cost) straight from
//! `ai_requests`. The bottom half describes what we think of it: judge runs,
//! per-item scores with the judge's rationale, the golden set, and pairwise
//! win rates.
//!
//! Runs are launched from here by POST and execute inline, so the redirect
//! back to the page already reflects the finished run. That is deliberate for
//! sample sizes in the tens; the `sample_size` ceiling in
//! [`crate::services::evals::MAX_SAMPLE_SIZE`] is what keeps it honest.

use std::sync::Arc;

use axum::extract::{Extension, Path, Query, State};
use axum::response::Response;
use serde::Deserialize;
use sqlx::PgPool;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::evals::{results, runs};
use crate::services::evals::MAX_SAMPLE_SIZE;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

mod actions;
mod context;
mod data;
mod format;
mod view;
mod view_runs;

use context::{EvalsPageContext, NoticeView, RunDetailContext};

use actions::require_admin;
pub(crate) use actions::{eval_promote_case_action, eval_run_action};

const BASE_URL: &str = "/admin/evals";
const DEFAULT_SAMPLE_SIZE: i64 = 20;
const RUN_DETAIL_RESULT_LIMIT: i64 = 200;

#[derive(Debug, Deserialize)]
pub(crate) struct EvalsQuery {
    pub from: Option<String>,
    pub to: Option<String>,
    pub preset: Option<String>,
    pub notice: Option<String>,
    pub notice_error: Option<String>,
}

pub(crate) async fn evals_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Query(query): Query<EvalsQuery>,
) -> AdminHtmlResult<Response> {
    require_admin(&user_ctx)?;

    let (range, auto_widened) = data::resolve_range(&pool, &query).await;
    let fetched = data::fetch_evals_data(&pool, range).await;

    let (histogram, histogram_max) = view::latency_buckets(&fetched.hist);
    let (cost_series, cost_max) = view::cost_buckets(&fetched.cost);
    let traffic = view::traffic_stats(&fetched.stats, &fetched.models, &fetched.users);
    let total = fetched.stats.total;

    let models = view::model_rows(&fetched.models, &fetched.model_scores, total);
    let users = view::user_rows(&fetched.users, total);
    let topics = view::topic_rows(&fetched.topics, total);
    let win_rates = view::win_rate_rows(&fetched.win_rates);
    let run_views = view_runs::run_rows(&fetched.runs);
    let result_views = view_runs::result_rows(&fetched.results);
    let case_views = view_runs::case_rows(&fetched.cases);
    let model_options = view::model_options(&fetched.models);

    let ctx = EvalsPageContext {
        page: "evals",
        title: "Evals",
        time_range: view::time_range_context(&query, &range, auto_widened),
        traffic,
        scores: view::score_summary(&fetched.scores, total),
        histogram,
        histogram_max,
        cost_series,
        cost_max,
        models,
        users,
        topics,
        win_rates,
        runs: run_views,
        results: result_views,
        cases: case_views,
        model_options,
        judge_model: default_judge_label(&fetched.models),
        default_sample_size: DEFAULT_SAMPLE_SIZE,
        max_sample_size: MAX_SAMPLE_SIZE,
        base_url: BASE_URL,
        notice: notice_from_query(&query),
    };

    Ok(super::render_typed_page(
        &engine, "evals", &ctx, &user_ctx, &mkt_ctx,
    ))
}

pub(crate) async fn eval_run_detail_page(
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    State(pool): State<Arc<PgPool>>,
    Path(run_id): Path<String>,
) -> AdminHtmlResult<Response> {
    require_admin(&user_ctx)?;

    let Some(run) = runs::find_run(&pool, &run_id)
        .await
        .map_err(AdminError::from)?
    else {
        return Err(AdminError::NotFound("No eval run with that id.".to_owned()).into());
    };

    let rows = results::list_results_for_run(&pool, &run_id, RUN_DETAIL_RESULT_LIMIT)
        .await
        .map_err(AdminError::from)?;
    let result_views = view_runs::result_rows(&rows);

    let ctx = RunDetailContext {
        page: "eval-run-detail",
        title: format!("Eval run · {}", run_id.chars().take(14).collect::<String>()),
        run: view_runs::run_row(&run),
        results: result_views,
        back_url: BASE_URL,
    };

    Ok(super::render_typed_page(
        &engine,
        "eval-run-detail",
        &ctx,
        &user_ctx,
        &mkt_ctx,
    ))
}


fn notice_from_query(query: &EvalsQuery) -> Option<NoticeView> {
    let message = query.notice.clone().filter(|n| !n.is_empty())?;
    Some(NoticeView {
        is_error: query.notice_error.as_deref() == Some("1"),
        message,
    })
}

/// Judge selection defaults to the least-used model in the window: the one
/// least likely to be grading its own output, which is the bias the default
/// most needs to avoid.
fn default_judge_label(
    models: &[crate::repositories::evals::distribution::ModelDistributionRow],
) -> String {
    models
        .iter()
        .min_by_key(|m| m.request_count)
        .map_or_else(|| "none available".to_owned(), |m| m.model.clone())
}
