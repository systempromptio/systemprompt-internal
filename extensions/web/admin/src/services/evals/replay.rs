//! Golden-set replay.
//!
//! Re-sends each frozen case to a target model and scores the fresh answer,
//! then compares it to the baseline answer recorded when the case was
//! promoted. That second step is what makes this a regression test rather
//! than just another judge run: a model change that quietly makes answers
//! worse shows up as baseline-wins, even when the absolute score still reads
//! "pass".

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt::ai::{AiMessage, AiRequest, AiService};

use crate::repositories::evals::cases::EvalCaseRow;
use crate::repositories::evals::{PairWinner, results};

use super::judge::JudgeConfig;
use super::{EXCERPT_CHARS, MAX_JUDGE_CHARS, ModelRef, RunTally, extract, judge, new_id};

/// Output-token budget for a replayed answer. Generous enough for a real
/// coding answer, bounded enough that a runaway case cannot dominate the bill.
const REPLAY_MAX_TOKENS: u32 = 4096;

pub struct ReplayParams<'a> {
    pub pool: &'a PgPool,
    pub ai: &'a Arc<AiService>,
    pub config: &'a JudgeConfig,
    pub run_id: &'a str,
    pub cases: &'a [EvalCaseRow],
    pub target: &'a ModelRef,
}

pub async fn execute_replay(params: ReplayParams<'_>) -> Result<RunTally, sqlx::Error> {
    let mut tally = RunTally::default();

    for case in params.cases {
        let Some(prompt) = extract::final_user_prompt(Some(&case.prompt_body)) else {
            tally.failed += 1;
            continue;
        };

        let Some(answer) = answer_for(params.ai, params.config, params.target, &prompt).await else {
            tally.failed += 1;
            continue;
        };

        let judged = judge::judge_answer(
            params.ai,
            params.pool,
            params.config,
            &extract::truncate_for_judge(&expectation_prompt(case, &prompt), MAX_JUDGE_CHARS),
            &extract::truncate_for_judge(&answer, MAX_JUDGE_CHARS),
        )
        .await;

        let Some(judged) = judged else {
            tally.failed += 1;
            continue;
        };
        tally.cost += judged.cost_microdollars;

        results::insert_result(
            params.pool,
            results::InsertResultParams {
                id: &new_id("evres"),
                run_id: params.run_id,
                ai_request_id: None,
                case_id: Some(&case.id),
                user_id: Some(&params.config.actor_user_id),
                session_id: None,
                provider: &params.target.provider,
                model: &params.target.model,
                overall_score: Some(i32::from(judged.verdict.overall_score)),
                dimension_scores: judged.verdict.dimension_scores(),
                verdict: super::parse_verdict(&judged.verdict.verdict),
                rationale: Some(&judged.verdict.rationale),
                flags: &judged.verdict.flags,
                prompt_excerpt: Some(&extract::excerpt(&prompt, EXCERPT_CHARS)),
                response_excerpt: Some(&extract::excerpt(&answer, EXCERPT_CHARS)),
                latency_ms: None,
                cost_microdollars: 0,
                judge_cost_microdollars: judged.cost_microdollars,
            },
        )
        .await?;
        tally.scored += 1;

        // Regression half: new answer against the answer this case was frozen
        // with. Skipped when the case has no baseline to regress against.
        if let Some(baseline) = case
            .baseline_response
            .as_ref()
            .and_then(|b| extract::assistant_answer(Some(b)))
        {
            let baseline_model = case.baseline_model.clone().unwrap_or_else(|| "baseline".into());
            if let Some(pair) = judge::judge_pair(
                params.ai,
                params.pool,
                params.config,
                &extract::truncate_for_judge(&prompt, MAX_JUDGE_CHARS),
                &extract::truncate_for_judge(&baseline, MAX_JUDGE_CHARS),
                &extract::truncate_for_judge(&answer, MAX_JUDGE_CHARS),
            )
            .await
            {
                tally.cost += pair.cost_microdollars;
                results::insert_pair(
                    params.pool,
                    results::InsertPairParams {
                        id: &new_id("evpair"),
                        run_id: params.run_id,
                        case_id: Some(&case.id),
                        model_a: &baseline_model,
                        model_b: &params.target.model,
                        winner: parse_winner(&pair.verdict.winner),
                        order_swapped: false,
                        rationale: Some(&pair.verdict.rationale),
                    },
                )
                .await?;
            }
        }
    }

    Ok(tally)
}

/// Send one case's prompt to a model and return the answer text. Shared with
/// [`super::pairwise`], which needs the same call for both contenders.
pub(super) async fn answer_for(
    ai: &Arc<AiService>,
    config: &JudgeConfig,
    target: &ModelRef,
    prompt: &str,
) -> Option<String> {
    let request = AiRequest::builder(
        vec![AiMessage::user(prompt)],
        &target.provider,
        &target.model,
        REPLAY_MAX_TOKENS,
        config.replay_context(&target.model),
    )
    .build();

    ai.generate(&request)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, model = %target.model, "replay call failed"))
        .ok()
        .map(|r| r.content)
        .filter(|c| !c.trim().is_empty())
}

/// The judge sees the case's stated expectation when it has one, so a replay
/// grades against the author's intent rather than against the prompt alone.
fn expectation_prompt(case: &EvalCaseRow, prompt: &str) -> String {
    case.expectation.as_deref().map_or_else(
        || prompt.to_owned(),
        |e| format!("{prompt}\n\n=== REVIEWER EXPECTATION ===\n{e}"),
    )
}

pub(super) fn parse_winner(s: &str) -> PairWinner {
    match s {
        "a" => PairWinner::A,
        "b" => PairWinner::B,
        _ => PairWinner::Tie,
    }
}
