//! `eval_results` and `eval_pairs` writes and reads.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::{SessionId, UserId};

use super::{EvalVerdict, PairWinner};
use crate::util::time_range::TimeRange;

#[derive(Debug, Clone, Serialize)]
pub struct EvalResultRow {
    pub id: String,
    pub run_id: String,
    pub ai_request_id: Option<String>,
    pub case_id: Option<String>,
    pub user_id: Option<UserId>,
    pub session_id: Option<SessionId>,
    pub provider: String,
    pub model: String,
    pub overall_score: Option<i32>,
    pub dimension_scores: serde_json::Value,
    pub verdict: String,
    pub rationale: Option<String>,
    pub flags: Vec<String>,
    pub prompt_excerpt: Option<String>,
    pub response_excerpt: Option<String>,
    pub latency_ms: Option<i32>,
    pub cost_microdollars: i64,
    pub judge_cost_microdollars: i64,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct InsertResultParams<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub ai_request_id: Option<&'a str>,
    pub case_id: Option<&'a str>,
    pub user_id: Option<&'a UserId>,
    pub session_id: Option<&'a SessionId>,
    pub provider: &'a str,
    pub model: &'a str,
    pub overall_score: Option<i32>,
    pub dimension_scores: serde_json::Value,
    pub verdict: EvalVerdict,
    pub rationale: Option<&'a str>,
    pub flags: &'a [String],
    pub prompt_excerpt: Option<&'a str>,
    pub response_excerpt: Option<&'a str>,
    pub latency_ms: Option<i32>,
    pub cost_microdollars: i64,
    pub judge_cost_microdollars: i64,
}

pub async fn insert_result(
    pool: &PgPool,
    params: InsertResultParams<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO eval_results
            (id, run_id, ai_request_id, case_id, user_id, session_id, provider, model,
             overall_score, dimension_scores, verdict, rationale, flags,
             prompt_excerpt, response_excerpt, latency_ms,
             cost_microdollars, judge_cost_microdollars)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18)
           ON CONFLICT DO NOTHING"#,
        params.id,
        params.run_id,
        params.ai_request_id,
        params.case_id,
        params.user_id.map(UserId::as_str),
        params.session_id.map(SessionId::as_str),
        params.provider,
        params.model,
        params.overall_score,
        params.dimension_scores,
        params.verdict.as_str(),
        params.rationale,
        params.flags,
        params.prompt_excerpt,
        params.response_excerpt,
        params.latency_ms,
        params.cost_microdollars,
        params.judge_cost_microdollars,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_results_for_run(
    pool: &PgPool,
    run_id: &str,
    limit: i64,
) -> Result<Vec<EvalResultRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        EvalResultRow,
        r#"SELECT
            id AS "id!",
            run_id AS "run_id!",
            ai_request_id,
            case_id,
            user_id AS "user_id?: UserId",
            session_id AS "session_id?: SessionId",
            provider AS "provider!",
            model AS "model!",
            overall_score,
            dimension_scores AS "dimension_scores!",
            verdict AS "verdict!",
            rationale,
            flags AS "flags!",
            prompt_excerpt,
            response_excerpt,
            latency_ms,
            cost_microdollars AS "cost_microdollars!",
            judge_cost_microdollars AS "judge_cost_microdollars!",
            created_at AS "created_at!"
          FROM eval_results
          WHERE run_id = $1
          ORDER BY overall_score ASC NULLS FIRST, created_at DESC
          LIMIT $2"#,
        run_id,
        limit,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

/// Most recent scored items across all runs in the window — the "worst first"
/// list the page leads with, since a passing answer needs no attention.
pub async fn list_recent_results(
    pool: &PgPool,
    range: TimeRange,
    limit: i64,
) -> Result<Vec<EvalResultRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        EvalResultRow,
        r#"SELECT
            id AS "id!",
            run_id AS "run_id!",
            ai_request_id,
            case_id,
            user_id AS "user_id?: UserId",
            session_id AS "session_id?: SessionId",
            provider AS "provider!",
            model AS "model!",
            overall_score,
            dimension_scores AS "dimension_scores!",
            verdict AS "verdict!",
            rationale,
            flags AS "flags!",
            prompt_excerpt,
            response_excerpt,
            latency_ms,
            cost_microdollars AS "cost_microdollars!",
            judge_cost_microdollars AS "judge_cost_microdollars!",
            created_at AS "created_at!"
          FROM eval_results
          WHERE created_at >= $1 AND created_at < $2
          ORDER BY
            CASE verdict WHEN 'fail' THEN 0 WHEN 'partial' THEN 1 ELSE 2 END,
            created_at DESC
          LIMIT $3"#,
        range.from,
        range.to,
        limit,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

#[derive(Debug)]
pub struct InsertPairParams<'a> {
    pub id: &'a str,
    pub run_id: &'a str,
    pub case_id: Option<&'a str>,
    pub model_a: &'a str,
    pub model_b: &'a str,
    pub winner: PairWinner,
    pub order_swapped: bool,
    pub rationale: Option<&'a str>,
}

pub async fn insert_pair(pool: &PgPool, params: InsertPairParams<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO eval_pairs
            (id, run_id, case_id, model_a, model_b, winner, order_swapped, rationale)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8)"#,
        params.id,
        params.run_id,
        params.case_id,
        params.model_a,
        params.model_b,
        params.winner.as_str(),
        params.order_swapped,
        params.rationale,
    )
    .execute(pool)
    .await?;
    Ok(())
}
