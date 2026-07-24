//! `eval_runs` writes and reads.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use sqlx::types::Json;
use systemprompt::identifiers::UserId;

use super::{EvalRunKind, EvalRunStatus};
use crate::util::time_range::TimeRange;

#[derive(Debug, Clone, Serialize)]
pub struct EvalRunRow {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub judge_provider: String,
    pub judge_model: String,
    pub sample_size: i32,
    pub scored_count: i32,
    pub failed_count: i32,
    pub cost_microdollars: i64,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub error_message: Option<String>,
    pub mean_score: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalRunFilterSnapshot {
    pub from: DateTime<Utc>,
    pub to: DateTime<Utc>,
    #[serde(default)]
    pub user_id: Option<UserId>,
    #[serde(default)]
    pub model: Option<String>,
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub compare_models: Vec<String>,
}

#[derive(Debug)]
pub struct InsertRunParams<'a> {
    pub id: &'a str,
    pub kind: EvalRunKind,
    pub judge_provider: &'a str,
    pub judge_model: &'a str,
    pub filter: Json<EvalRunFilterSnapshot>,
    pub sample_size: i32,
    pub created_by: &'a str,
}

pub async fn insert_run(pool: &PgPool, params: InsertRunParams<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO eval_runs
            (id, kind, status, judge_provider, judge_model, filter, sample_size, created_by)
           VALUES ($1, $2, 'running', $3, $4, $5, $6, $7)"#,
        params.id,
        params.kind.as_str(),
        params.judge_provider,
        params.judge_model,
        params.filter as _,
        params.sample_size,
        params.created_by,
    )
    .execute(pool)
    .await?;
    Ok(())
}

#[derive(Debug)]
pub struct CompleteRunParams<'a> {
    pub id: &'a str,
    pub status: EvalRunStatus,
    pub scored_count: i32,
    pub failed_count: i32,
    pub cost_microdollars: i64,
    pub error_message: Option<&'a str>,
}

pub async fn update_run_completion(
    pool: &PgPool,
    params: CompleteRunParams<'_>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"UPDATE eval_runs
           SET status = $2,
               scored_count = $3,
               failed_count = $4,
               cost_microdollars = $5,
               error_message = $6,
               completed_at = CURRENT_TIMESTAMP
           WHERE id = $1"#,
        params.id,
        params.status.as_str(),
        params.scored_count,
        params.failed_count,
        params.cost_microdollars,
        params.error_message,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_recent_runs(
    pool: &PgPool,
    range: TimeRange,
    limit: i64,
) -> Result<Vec<EvalRunRow>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"SELECT
            r.id AS "id!",
            r.kind AS "kind!",
            r.status AS "status!",
            r.judge_provider AS "judge_provider!",
            r.judge_model AS "judge_model!",
            r.sample_size AS "sample_size!",
            r.scored_count AS "scored_count!",
            r.failed_count AS "failed_count!",
            r.cost_microdollars AS "cost_microdollars!",
            r.created_by AS "created_by!",
            r.created_at AS "created_at!",
            r.completed_at,
            r.error_message,
            (SELECT AVG(overall_score)::float8 FROM eval_results er WHERE er.run_id = r.id)
                AS mean_score
          FROM eval_runs r
          WHERE r.created_at >= $1 AND r.created_at < $2
          ORDER BY r.created_at DESC
          LIMIT $3"#,
        range.from,
        range.to,
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EvalRunRow {
            id: r.id,
            kind: r.kind,
            status: r.status,
            judge_provider: r.judge_provider,
            judge_model: r.judge_model,
            sample_size: r.sample_size,
            scored_count: r.scored_count,
            failed_count: r.failed_count,
            cost_microdollars: r.cost_microdollars,
            created_by: r.created_by,
            created_at: r.created_at,
            completed_at: r.completed_at,
            error_message: r.error_message,
            mean_score: r.mean_score,
        })
        .collect())
}

pub async fn find_run(pool: &PgPool, run_id: &str) -> Result<Option<EvalRunRow>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT
            r.id AS "id!",
            r.kind AS "kind!",
            r.status AS "status!",
            r.judge_provider AS "judge_provider!",
            r.judge_model AS "judge_model!",
            r.sample_size AS "sample_size!",
            r.scored_count AS "scored_count!",
            r.failed_count AS "failed_count!",
            r.cost_microdollars AS "cost_microdollars!",
            r.created_by AS "created_by!",
            r.created_at AS "created_at!",
            r.completed_at,
            r.error_message,
            (SELECT AVG(overall_score)::float8 FROM eval_results er WHERE er.run_id = r.id)
                AS mean_score
          FROM eval_runs r
          WHERE r.id = $1"#,
        run_id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| EvalRunRow {
        id: r.id,
        kind: r.kind,
        status: r.status,
        judge_provider: r.judge_provider,
        judge_model: r.judge_model,
        sample_size: r.sample_size,
        scored_count: r.scored_count,
        failed_count: r.failed_count,
        cost_microdollars: r.cost_microdollars,
        created_by: r.created_by,
        created_at: r.created_at,
        completed_at: r.completed_at,
        error_message: r.error_message,
        mean_score: r.mean_score,
    }))
}
