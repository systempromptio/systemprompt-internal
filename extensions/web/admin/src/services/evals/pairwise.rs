//! Head-to-head model comparison.
//!
//! Each case is answered by both models, then judged twice — once with A
//! presented first, once with B presented first. Only an agreeing pair counts
//! as a decisive win; a disagreement is recorded as a tie with
//! `order_swapped` set, which is exactly the signal that the judge was reading
//! position rather than content. Storing both rows keeps that measurable
//! instead of averaging it away.

use sqlx::PgPool;

use crate::repositories::evals::cases::EvalCaseRow;
use crate::repositories::evals::{PairWinner, results};

use super::judge::JudgeConfig;
use super::replay::parse_winner;
use super::{MAX_JUDGE_CHARS, ModelRef, RunTally, extract, judge, new_id, replay};

pub(crate) struct PairwiseParams<'a> {
    pub pool: &'a PgPool,
    pub config: &'a JudgeConfig,
    pub run_id: &'a str,
    pub cases: &'a [EvalCaseRow],
    pub model_a: &'a ModelRef,
    pub model_b: &'a ModelRef,
}

pub(crate) async fn execute_pairwise(params: PairwiseParams<'_>) -> Result<RunTally, sqlx::Error> {
    let mut tally = RunTally::default();

    for case in params.cases {
        let Some(prompt) = extract::final_user_prompt(Some(&case.prompt_body)) else {
            tally.failed += 1;
            continue;
        };
        let prompt = extract::truncate_for_judge(&prompt, MAX_JUDGE_CHARS);

        let (answer_a, answer_b) = tokio::join!(
            replay::answer_for(params.config, params.model_a, &prompt),
            replay::answer_for(params.config, params.model_b, &prompt),
        );

        let (Some(answer_a), Some(answer_b)) = (answer_a, answer_b) else {
            tally.failed += 1;
            continue;
        };
        let answer_a = extract::truncate_for_judge(&answer_a, MAX_JUDGE_CHARS);
        let answer_b = extract::truncate_for_judge(&answer_b, MAX_JUDGE_CHARS);

        let forward = judge::judge_pair(judge::PairParams {
            pool: params.pool,
            config: params.config,
            prompt: &prompt,
            answer_a: &answer_a,
            answer_b: &answer_b,
        })
        .await;
        // Why: swapped order (B first) so a position-biased judge contradicts itself.
        let reverse = judge::judge_pair(judge::PairParams {
            pool: params.pool,
            config: params.config,
            prompt: &prompt,
            answer_a: &answer_b,
            answer_b: &answer_a,
        })
        .await;

        let (Some(forward), Some(reverse)) = (forward, reverse) else {
            tally.failed += 1;
            continue;
        };
        tally.cost += forward.cost_microdollars + reverse.cost_microdollars;

        let forward_winner = parse_winner(&forward.verdict.winner);
        // Why: in the swapped run the labels mean the opposite models.
        let reverse_winner = flip(parse_winner(&reverse.verdict.winner));
        let agreed = forward_winner == reverse_winner;

        for (winner, swapped, rationale) in [
            (
                if agreed { forward_winner } else { PairWinner::Tie },
                false,
                forward.verdict.rationale.as_str(),
            ),
            (
                if agreed { reverse_winner } else { PairWinner::Tie },
                true,
                reverse.verdict.rationale.as_str(),
            ),
        ] {
            results::insert_pair(
                params.pool,
                results::InsertPairParams {
                    id: &new_id("evpair"),
                    run_id: params.run_id,
                    case_id: Some(&case.id),
                    model_a: &params.model_a.model,
                    model_b: &params.model_b.model,
                    winner,
                    order_swapped: swapped,
                    rationale: Some(rationale),
                },
            )
            .await?;
        }

        tally.scored += 1;
    }

    Ok(tally)
}

const fn flip(w: PairWinner) -> PairWinner {
    match w {
        PairWinner::A => PairWinner::B,
        PairWinner::B => PairWinner::A,
        PairWinner::Tie => PairWinner::Tie,
    }
}
