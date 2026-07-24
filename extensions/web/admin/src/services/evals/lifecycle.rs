//! Run bookkeeping shared by every eval kind: open/close the `eval_runs`
//! row, mint ids, and parse judge verdict strings.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::repositories::evals::{EvalRunKind, EvalRunStatus, EvalVerdict, runs};

use super::judge::JudgeConfig;
use super::{EvalRunRequest, ModelRef};

/// Per-run tallies shared by the replay and pairwise executors.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunTally {
    pub scored: i32,
    pub failed: i32,
    pub cost: i64,
}

pub(crate) async fn open_run(
    pool: &PgPool,
    run_id: &str,
    kind: EvalRunKind,
    config: &JudgeConfig,
    request: &EvalRunRequest,
    sample_size: usize,
) -> Result<(), sqlx::Error> {
    runs::insert_run(
        pool,
        runs::InsertRunParams {
            id: run_id,
            kind,
            judge_provider: &config.provider,
            judge_model: &config.model,
            filter: serde_json::json!({
                "from": request.range.from,
                "to": request.range.to,
                "user_id": request.filter.user_id.as_ref().map(UserId::as_str),
                "model": request.filter.model,
                "provider": request.filter.provider,
                "compare_models": request.compare_models
                    .iter()
                    .map(ModelRef::as_value)
                    .collect::<Vec<_>>(),
            }),
            sample_size: i32::try_from(sample_size).unwrap_or(i32::MAX),
            created_by: request.actor.as_str(),
        },
    )
    .await
}

pub(crate) async fn close_run(
    pool: &PgPool,
    run_id: &str,
    scored: i32,
    failed: i32,
    cost: i64,
) -> Result<(), sqlx::Error> {
    runs::update_run_completion(
        pool,
        runs::CompleteRunParams {
            id: run_id,
            status: if scored == 0 && failed > 0 {
                EvalRunStatus::Failed
            } else {
                EvalRunStatus::Completed
            },
            scored_count: scored,
            failed_count: failed,
            cost_microdollars: cost,
            error_message: None,
        },
    )
    .await
}

pub(crate) fn parse_verdict(s: &str) -> EvalVerdict {
    match s {
        "pass" => EvalVerdict::Pass,
        "partial" => EvalVerdict::Partial,
        _ => EvalVerdict::Fail,
    }
}

pub(crate) fn new_id(prefix: &str) -> String {
    format!("{prefix}_{}", uuid::Uuid::new_v4().simple())
}
