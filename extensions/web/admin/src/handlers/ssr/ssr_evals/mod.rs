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

use axum::Form;
use axum::extract::{Extension, Path, Query, State};
use axum::response::{Redirect, Response};
use serde::Deserialize;
use sqlx::PgPool;
use systemprompt::ai::AiService;

use crate::error::{AdminError, AdminHtmlResult};
use crate::repositories::evals::sampling::CandidateFilter;
use crate::repositories::evals::{EvalRunKind, results, runs};
use crate::services::evals::{
    self, EvalError, EvalRunOutcome, EvalRunRequest, MAX_SAMPLE_SIZE, ModelRef,
};
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

mod context;
mod data;
mod format;
mod view;
mod view_runs;

use context::{EvalsPageContext, NoticeView, RunDetailContext};

pub(self) const BASE_URL: &str = "/admin/evals";
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
    Extension(ai_service): Extension<Option<Arc<AiService>>>,
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
        has_models: !models.is_empty(),
        models,
        has_users: !users.is_empty(),
        users,
        has_topics: !topics.is_empty(),
        topics,
        has_win_rates: !win_rates.is_empty(),
        win_rates,
        has_runs: !run_views.is_empty(),
        runs: run_views,
        has_results: !result_views.is_empty(),
        results: result_views,
        has_cases: !case_views.is_empty(),
        cases: case_views,
        model_options,
        judge_model: ai_service
            .as_ref()
            .map_or_else(|| "unavailable".to_owned(), |ai| ai.default_model().to_owned()),
        default_sample_size: DEFAULT_SAMPLE_SIZE,
        max_sample_size: MAX_SAMPLE_SIZE,
        base_url: BASE_URL,
        notice: notice_from_query(&query),
    };

    Ok(super::render_typed_page(
        &engine,
        "evals",
        &ctx,
        &user_ctx,
        &mkt_ctx,
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

    let Some(run) = runs::find_run(&pool, &run_id).await.map_err(AdminError::from)? else {
        return Err(AdminError::NotFound("No eval run with that id.".to_owned()).into());
    };

    let rows = results::list_results_for_run(&pool, &run_id, RUN_DETAIL_RESULT_LIMIT)
        .await
        .map_err(AdminError::from)?;
    let result_views = view_runs::result_rows(&rows);

    let ctx = RunDetailContext {
        page: "eval-run-detail",
        title: format!("Eval run · {}", &run_id.chars().take(14).collect::<String>()),
        run: view_runs::run_row(&run),
        has_results: !result_views.is_empty(),
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

#[derive(Debug, Deserialize)]
pub(crate) struct RunEvalForm {
    pub kind: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub sample_size: Option<i64>,
    pub model: Option<String>,
    pub model_a: Option<String>,
    pub model_b: Option<String>,
}

pub(crate) async fn eval_run_action(
    Extension(user_ctx): Extension<UserContext>,
    Extension(ai_service): Extension<Option<Arc<AiService>>>,
    State(pool): State<Arc<PgPool>>,
    Form(form): Form<RunEvalForm>,
) -> AdminHtmlResult<Redirect> {
    require_admin(&user_ctx)?;

    let range = data::range_from_strings(form.from.as_deref(), form.to.as_deref());

    let Some(ai) = ai_service else {
        return Ok(Redirect::to(&view::redirect_url(
            &range,
            "AI service is not available, so no judge could be run.",
            true,
        )));
    };

    let kind = EvalRunKind::from_str_opt(&form.kind).unwrap_or(EvalRunKind::Judge);
    let compare_models = [form.model_a.as_deref(), form.model_b.as_deref()]
        .into_iter()
        .chain(std::iter::once(form.model.as_deref()))
        .flatten()
        .filter_map(ModelRef::parse)
        .collect::<Vec<_>>();

    let request = EvalRunRequest {
        kind,
        range,
        filter: CandidateFilter::default(),
        sample_size: form.sample_size.unwrap_or(DEFAULT_SAMPLE_SIZE),
        actor: user_ctx.user_id.clone(),
        compare_models,
    };

    let outcome = match kind {
        EvalRunKind::Judge => evals::run_judge_eval(&pool, &ai, &request).await,
        EvalRunKind::Replay => evals::run_replay_eval(&pool, &ai, &request).await,
        EvalRunKind::Pairwise => evals::run_pairwise_eval(&pool, &ai, &request).await,
    };

    Ok(Redirect::to(&run_redirect(&range, kind, outcome)))
}

fn run_redirect(
    range: &crate::util::time_range::TimeRange,
    kind: EvalRunKind,
    outcome: Result<EvalRunOutcome, EvalError>,
) -> String {
    match outcome {
        Ok(o) => view::redirect_url(
            range,
            &format!(
                "{} run finished: {} scored, {} failed, judge cost ${:.4}.",
                kind.as_str(),
                o.scored,
                o.failed,
                o.cost_microdollars as f64 / 1_000_000.0,
            ),
            false,
        ),
        Err(e) => {
            tracing::warn!(error = %e, kind = kind.as_str(), "eval run failed");
            view::redirect_url(range, &format!("Eval run failed: {e}"), true)
        },
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PromoteCaseForm {
    pub ai_request_id: String,
    pub name: Option<String>,
    pub expectation: Option<String>,
    pub from: Option<String>,
    pub to: Option<String>,
}

pub(crate) async fn eval_promote_case_action(
    Extension(user_ctx): Extension<UserContext>,
    State(pool): State<Arc<PgPool>>,
    Form(form): Form<PromoteCaseForm>,
) -> AdminHtmlResult<Redirect> {
    require_admin(&user_ctx)?;

    let range = data::range_from_strings(form.from.as_deref(), form.to.as_deref());
    let outcome = evals::promote_case(
        &pool,
        &form.ai_request_id,
        form.name.as_deref(),
        form.expectation.as_deref().filter(|e| !e.trim().is_empty()),
        &user_ctx.user_id,
    )
    .await;

    let url = match outcome {
        Ok(_) => view::redirect_url(&range, "Added to the golden set.", false),
        Err(e) => {
            tracing::warn!(error = %e, "promoting eval case failed");
            view::redirect_url(&range, &format!("Could not add to the golden set: {e}"), true)
        },
    };
    Ok(Redirect::to(&url))
}

fn require_admin(user_ctx: &UserContext) -> Result<(), crate::error::AdminHtmlError> {
    if user_ctx.is_admin {
        Ok(())
    } else {
        Err(AdminError::Forbidden("Admin access required.".to_owned()).into())
    }
}

fn notice_from_query(query: &EvalsQuery) -> Option<NoticeView> {
    let message = query.notice.clone().filter(|n| !n.is_empty())?;
    Some(NoticeView {
        is_error: query.notice_error.as_deref() == Some("1"),
        message,
    })
}
