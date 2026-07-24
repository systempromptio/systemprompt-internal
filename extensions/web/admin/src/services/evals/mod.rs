//! Evaluation engine.
//!
//! Three run kinds, one shared judge:
//!
//! - [`run_judge_eval`] scores real gateway traffic reference-free.
//! - [`run_replay_eval`] re-sends the golden set and scores the fresh answers.
//! - [`run_pairwise_eval`] puts two models on the same case and picks a winner.
//!
//! Every run writes an `eval_runs` row first and closes it out at the end, so
//! a crashed run is visible as `running` with no completion rather than
//! silently absent. Judge calls go through `AiService`, which means they are
//! themselves governed, audited, and costed — and are excluded from future
//! candidate pools by [`crate::repositories::evals::sampling`].

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt::ai::AiService;
use systemprompt::identifiers::UserId;

pub mod deterministic;
pub mod judge_run;
mod lifecycle;
pub mod extract;
pub mod judge;
pub mod pairwise;
pub mod replay;
pub mod rubric;

use crate::repositories::evals::sampling::CandidateFilter;
use crate::repositories::evals::{EvalRunKind, cases, sampling};
use crate::util::time_range::TimeRange;

pub use judge_run::run_judge_eval;
pub use lifecycle::RunTally;
pub(crate) use lifecycle::{close_run, new_id, open_run, parse_verdict};

use judge::JudgeConfig;

/// Longest prompt/answer text handed to a judge call.
pub(crate) const MAX_JUDGE_CHARS: usize = 8_000;
/// Length of the excerpts stored on each result row for the table.
pub(crate) const EXCERPT_CHARS: usize = 240;
/// Ceiling on how many items one run may score, whatever the caller asks for.
pub const MAX_SAMPLE_SIZE: i64 = 200;

/// A model and the provider that serves it. A bare model id is not enough to
/// place a call, so replay and pairwise carry both rather than re-deriving the
/// provider from a naming convention.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRef {
    pub provider: String,
    pub model: String,
}

impl ModelRef {
    /// Parse the `provider/model` form used by the page's select controls.
    /// A slashless input is rejected outright: silently attributing a bare
    /// model id to some default provider would bill the wrong upstream.
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        let (provider, model) = s.split_once('/')?;
        (!provider.is_empty() && !model.is_empty()).then(|| Self {
            provider: provider.to_owned(),
            model: model.to_owned(),
        })
    }

    #[must_use]
    pub fn as_value(&self) -> String {
        format!("{}/{}", self.provider, self.model)
    }
}

/// What the caller asked for. Serialised onto `eval_runs.filter` so a run can
/// be read back later without guessing what it covered.
#[derive(Debug, Clone)]
pub struct EvalRunRequest {
    pub kind: EvalRunKind,
    pub range: TimeRange,
    pub filter: CandidateFilter,
    pub sample_size: i64,
    pub actor: UserId,
    /// Models under test. Replay uses the first; pairwise needs two.
    pub compare_models: Vec<ModelRef>,
}

/// Outcome summary handed back to the caller for the redirect.
#[derive(Debug, Clone)]
pub struct EvalRunOutcome {
    pub run_id: String,
    pub scored: i32,
    pub failed: i32,
    pub cost_microdollars: i64,
}

#[derive(Debug, thiserror::Error)]
pub enum EvalError {
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("no candidates matched the requested window and filters")]
    NoCandidates,
    #[error("golden set is empty — promote a request into it first")]
    NoCases,
    #[error("pairwise runs need two distinct models")]
    NeedTwoModels,
}

/// Re-run the golden set and score the fresh answers.
pub async fn run_replay_eval(
    pool: &PgPool,
    ai: &Arc<AiService>,
    request: &EvalRunRequest,
) -> Result<EvalRunOutcome, EvalError> {
    let run_id = new_id("evrun");
    let config = JudgeConfig::from_defaults(ai, request.actor.clone(), run_id.clone());

    let case_rows = cases::list_cases(pool, true).await?;
    if case_rows.is_empty() {
        return Err(EvalError::NoCases);
    }

    let target = request.compare_models.first().cloned().unwrap_or(ModelRef {
        provider: ai.default_provider().to_owned(),
        model: ai.default_model().to_owned(),
    });

    open_run(
        pool,
        &run_id,
        EvalRunKind::Replay,
        &config,
        request,
        case_rows.len(),
    )
    .await?;

    let outcome = replay::execute_replay(replay::ReplayParams {
        pool,
        ai,
        config: &config,
        run_id: &run_id,
        cases: &case_rows,
        target: &target,
    })
    .await?;

    close_run(pool, &run_id, outcome.scored, outcome.failed, outcome.cost).await?;

    Ok(EvalRunOutcome {
        run_id,
        scored: outcome.scored,
        failed: outcome.failed,
        cost_microdollars: outcome.cost,
    })
}

/// Put two models on every golden-set case and record who wins.
pub async fn run_pairwise_eval(
    pool: &PgPool,
    ai: &Arc<AiService>,
    request: &EvalRunRequest,
) -> Result<EvalRunOutcome, EvalError> {
    if request.compare_models.len() < 2 || request.compare_models[0] == request.compare_models[1] {
        return Err(EvalError::NeedTwoModels);
    }

    let run_id = new_id("evrun");
    let config = JudgeConfig::from_defaults(ai, request.actor.clone(), run_id.clone());

    let case_rows = cases::list_cases(pool, true).await?;
    if case_rows.is_empty() {
        return Err(EvalError::NoCases);
    }

    open_run(
        pool,
        &run_id,
        EvalRunKind::Pairwise,
        &config,
        request,
        case_rows.len(),
    )
    .await?;

    let outcome = pairwise::execute_pairwise(pairwise::PairwiseParams {
        pool,
        ai,
        config: &config,
        run_id: &run_id,
        cases: &case_rows,
        model_a: &request.compare_models[0],
        model_b: &request.compare_models[1],
    })
    .await?;

    close_run(pool, &run_id, outcome.scored, outcome.failed, outcome.cost).await?;

    Ok(EvalRunOutcome {
        run_id,
        scored: outcome.scored,
        failed: outcome.failed,
        cost_microdollars: outcome.cost,
    })
}

/// Freeze one live request into the golden set.
pub async fn promote_case(
    pool: &PgPool,
    ai_request_id: &str,
    name: Option<&str>,
    expectation: Option<&str>,
    actor: &UserId,
) -> Result<String, EvalError> {
    let Some(candidate) = sampling::find_candidate_by_id(pool, ai_request_id).await? else {
        return Err(EvalError::NoCandidates);
    };

    let prompt = extract::final_user_prompt(candidate.request_body.as_ref())
        .or_else(|| candidate.request_excerpt.clone())
        .unwrap_or_default();
    let derived_name = name
        .map(str::to_owned)
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| extract::excerpt(&prompt, 80));

    let case_id = new_id("evcase");
    let body = candidate
        .request_body
        .clone()
        .unwrap_or_else(|| serde_json::json!({ "messages": [] }));

    cases::insert_case(
        pool,
        cases::InsertCaseParams {
            id: &case_id,
            name: &derived_name,
            prompt_body: body,
            source_ai_request_id: Some(candidate.ai_request_id.as_str()),
            expectation,
            baseline_response: candidate.response_body.clone(),
            baseline_model: Some(&candidate.model),
            tags: &[],
            created_by: actor.as_str(),
        },
    )
    .await?;

    Ok(case_id)
}

