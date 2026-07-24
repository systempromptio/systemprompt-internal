//! Golden-set replay.
//!
//! Re-sends each frozen case to a target model and scores the fresh answer,
//! then compares it to the baseline answer recorded when the case was
//! promoted. That second step is what makes this a regression test rather
//! than just another judge run: a model change that quietly makes answers
//! worse shows up as baseline-wins, even when the absolute score still reads
//! "pass".

use sqlx::PgPool;

use crate::repositories::evals::cases::EvalCaseRow;
use crate::repositories::evals::{PairWinner, results};

use super::gateway_client;
use super::judge::JudgeConfig;
use super::{EXCERPT_CHARS, MAX_JUDGE_CHARS, ModelRef, RunTally, extract, judge, new_id};

/// Output-token budget for a replayed answer. Generous enough for a real
/// coding answer, bounded enough that a runaway case cannot dominate the bill.
const REPLAY_MAX_TOKENS: u32 = 4096;

pub(crate) struct ReplayParams<'a> {
    pub pool: &'a PgPool,
    pub config: &'a JudgeConfig,
    pub run_id: &'a str,
    pub cases: &'a [EvalCaseRow],
    pub target: &'a ModelRef,
}

pub(crate) async fn execute_replay(params: ReplayParams<'_>) -> Result<RunTally, sqlx::Error> {
    let mut tally = RunTally::default();
    for case in params.cases {
        replay_one(&params, case, &mut tally).await?;
    }
    Ok(tally)
}

/// One case: re-answer it, score the answer, then regress it against the
/// baseline the case was frozen with.
async fn replay_one(
    params: &ReplayParams<'_>,
    case: &EvalCaseRow,
    tally: &mut RunTally,
) -> Result<(), sqlx::Error> {
    let Some(prompt) = extract::final_user_prompt(Some(&case.prompt_body)) else {
        tally.failed += 1;
        return Ok(());
    };

    let Some(answer) = answer_for(params.config, params.target, &prompt).await else {
        tally.failed += 1;
        return Ok(());
    };

    let judged = judge::judge_answer(
        params.pool,
        params.config,
        &extract::truncate_for_judge(&expectation_prompt(case, &prompt), MAX_JUDGE_CHARS),
        &extract::truncate_for_judge(&answer, MAX_JUDGE_CHARS),
    )
    .await;

    let Some(judged) = judged else {
        tally.failed += 1;
        return Ok(());
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

    regress_against_baseline(params, case, &prompt, &answer, tally).await
}

/// Compare the fresh answer to the one the case was frozen with. Skipped when
/// the case has no baseline: there is nothing to regress against, and a
/// missing baseline is not a failure.
async fn regress_against_baseline(
    params: &ReplayParams<'_>,
    case: &EvalCaseRow,
    prompt: &str,
    answer: &str,
    tally: &mut RunTally,
) -> Result<(), sqlx::Error> {
    let Some(baseline) = case
        .baseline_response
        .as_ref()
        .and_then(|b| extract::assistant_answer(Some(b)))
    else {
        return Ok(());
    };

    let baseline_model = case
        .baseline_model
        .clone()
        .unwrap_or_else(|| "baseline".to_owned());

    let Some(pair) = judge::judge_pair(judge::PairParams {
        pool: params.pool,
        config: params.config,
        prompt: &extract::truncate_for_judge(prompt, MAX_JUDGE_CHARS),
        answer_a: &extract::truncate_for_judge(&baseline, MAX_JUDGE_CHARS),
        answer_b: &extract::truncate_for_judge(answer, MAX_JUDGE_CHARS),
    })
    .await
    else {
        return Ok(());
    };

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
    .await
}

pub(super) async fn answer_for(
    config: &JudgeConfig,
    target: &ModelRef,
    prompt: &str,
) -> Option<String> {
    gateway_client::call_messages(gateway_client::CallParams {
        credential: &config.credential,
        model: &target.model,
        system: None,
        user: prompt,
        max_tokens: REPLAY_MAX_TOKENS,
        conversation_id: &new_id("evreplay"),
    })
    .await
    .map(|a| a.text)
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
