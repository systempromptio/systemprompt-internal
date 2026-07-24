//! Reference-free judge run over live gateway traffic.
//!
//! Split from `mod.rs` so the module stays under the size ceiling; the run
//! lifecycle (open/close, ids, verdict parsing) lives in [`super::lifecycle`].

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt::ai::AiService;

use crate::repositories::evals::{EvalRunKind, EvalVerdict, results, sampling};

use super::lifecycle::{close_run, new_id, open_run, parse_verdict};
use super::{EXCERPT_CHARS, EvalError, EvalRunOutcome, EvalRunRequest, MAX_JUDGE_CHARS, MAX_SAMPLE_SIZE};
use super::{deterministic, extract, judge};
use judge::JudgeConfig;

/// Score a slice of live gateway traffic.
// lint-ok: unused-pub — consumed by the evals dashboard page currently in development
pub async fn run_judge_eval(
    pool: &PgPool,
    ai: &Arc<AiService>,
    request: &EvalRunRequest,
) -> Result<EvalRunOutcome, EvalError> {
    let run_id = new_id("evrun");
    let config = JudgeConfig::from_defaults(ai, request.actor.clone(), run_id.clone());
    let sample_size = request.sample_size.clamp(1, MAX_SAMPLE_SIZE);

    let mut filter = request.filter.clone();
    // Never re-score what this judge model has already scored: a second run
    // over the same window should extend coverage, not duplicate it.
    filter.skip_judged_by = Some(config.model.clone());

    let candidates =
        sampling::list_eval_candidates(pool, &filter, request.range, sample_size).await?;
    if candidates.is_empty() {
        return Err(EvalError::NoCandidates);
    }

    open_run(
        pool,
        &run_id,
        EvalRunKind::Judge,
        &config,
        request,
        candidates.len(),
    )
    .await?;

    let mut scored = 0i32;
    let mut failed = 0i32;
    let mut cost = 0i64;

    for candidate in candidates {
        let pre = deterministic::run_pre_pass(&candidate);
        let prompt_excerpt = pre
            .prompt
            .as_deref()
            .map(|p| extract::excerpt(p, EXCERPT_CHARS));
        let response_excerpt = pre
            .answer
            .as_deref()
            .map(|a| extract::excerpt(a, EXCERPT_CHARS));

        if let Some((verdict, rationale)) = pre.short_circuit {
            results::insert_result(
                pool,
                results::InsertResultParams {
                    id: &new_id("evres"),
                    run_id: &run_id,
                    ai_request_id: Some(candidate.ai_request_id.as_str()),
                    case_id: None,
                    user_id: Some(&candidate.user_id),
                    session_id: candidate.session_id.as_ref(),
                    provider: &candidate.provider,
                    model: &candidate.model,
                    overall_score: matches!(verdict, EvalVerdict::Fail).then_some(1),
                    dimension_scores: serde_json::json!({}),
                    verdict,
                    rationale: Some(&rationale),
                    flags: &pre.flags,
                    prompt_excerpt: prompt_excerpt.as_deref(),
                    response_excerpt: response_excerpt.as_deref(),
                    latency_ms: candidate.latency_ms,
                    cost_microdollars: candidate.cost_microdollars,
                    judge_cost_microdollars: 0,
                },
            )
            .await?;
            scored += 1;
            continue;
        }

        let (Some(prompt), Some(answer)) = (pre.prompt.as_deref(), pre.answer.as_deref()) else {
            failed += 1;
            continue;
        };

        let judged = judge::judge_answer(
            ai,
            pool,
            &config,
            &extract::truncate_for_judge(prompt, MAX_JUDGE_CHARS),
            &extract::truncate_for_judge(answer, MAX_JUDGE_CHARS),
        )
        .await;

        let Some(judged) = judged else {
            failed += 1;
            continue;
        };

        cost += judged.cost_microdollars;
        let mut flags = pre.flags.clone();
        for f in &judged.verdict.flags {
            if !flags.contains(f) {
                flags.push(f.clone());
            }
        }

        results::insert_result(
            pool,
            results::InsertResultParams {
                id: &new_id("evres"),
                run_id: &run_id,
                ai_request_id: Some(candidate.ai_request_id.as_str()),
                case_id: None,
                user_id: Some(&candidate.user_id),
                session_id: candidate.session_id.as_ref(),
                provider: &candidate.provider,
                model: &candidate.model,
                overall_score: Some(i32::from(judged.verdict.overall_score)),
                dimension_scores: judged.verdict.dimension_scores(),
                verdict: parse_verdict(&judged.verdict.verdict),
                rationale: Some(&judged.verdict.rationale),
                flags: &flags,
                prompt_excerpt: prompt_excerpt.as_deref(),
                response_excerpt: response_excerpt.as_deref(),
                latency_ms: candidate.latency_ms,
                cost_microdollars: candidate.cost_microdollars,
                judge_cost_microdollars: judged.cost_microdollars,
            },
        )
        .await?;
        scored += 1;
    }

    close_run(pool, &run_id, scored, failed, cost).await?;

    Ok(EvalRunOutcome {
        run_id,
        scored,
        failed,
        cost_microdollars: cost,
    })
}
