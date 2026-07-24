//! `eval_cases` — the golden set.
//!
//! A case is a frozen `/v1/messages` body promoted out of real traffic, plus
//! the answer the source model gave at the time. Replay runs re-send the body
//! and compare the fresh answer to that baseline.

use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;

#[derive(Debug, Clone, Serialize)]
pub struct EvalCaseRow {
    pub id: String,
    pub name: String,
    // JSON: frozen provider `/v1/messages` request body, replayed verbatim; its shape is the
    // upstream wire contract, not ours.
    pub prompt_body: serde_json::Value,
    pub source_ai_request_id: Option<String>,
    pub expectation: Option<String>,
    // JSON: frozen provider response body kept byte-for-byte as the replay baseline.
    pub baseline_response: Option<serde_json::Value>,
    pub baseline_model: Option<String>,
    pub tags: Vec<String>,
    pub enabled: bool,
    pub created_by: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct InsertCaseParams<'a> {
    pub id: &'a str,
    pub name: &'a str,
    // JSON: frozen provider `/v1/messages` request body, replayed verbatim; its shape is the
    // upstream wire contract, not ours.
    pub prompt_body: serde_json::Value,
    pub source_ai_request_id: Option<&'a str>,
    pub expectation: Option<&'a str>,
    // JSON: frozen provider response body kept byte-for-byte as the replay baseline.
    pub baseline_response: Option<serde_json::Value>,
    pub baseline_model: Option<&'a str>,
    pub tags: &'a [String],
    pub created_by: &'a str,
}

pub async fn insert_case(pool: &PgPool, params: InsertCaseParams<'_>) -> Result<(), sqlx::Error> {
    sqlx::query!(
        r#"INSERT INTO eval_cases
            (id, name, prompt_body, source_ai_request_id, expectation,
             baseline_response, baseline_model, tags, created_by)
           VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)"#,
        params.id,
        params.name,
        params.prompt_body,
        params.source_ai_request_id,
        params.expectation,
        params.baseline_response,
        params.baseline_model,
        params.tags,
        params.created_by,
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_cases(
    pool: &PgPool,
    enabled_only: bool,
) -> Result<Vec<EvalCaseRow>, sqlx::Error> {
    let rows = sqlx::query_as!(
        EvalCaseRow,
        r#"SELECT
            id AS "id!",
            name AS "name!",
            prompt_body AS "prompt_body!",
            source_ai_request_id,
            expectation,
            baseline_response,
            baseline_model,
            tags AS "tags!",
            enabled AS "enabled!",
            created_by AS "created_by!",
            created_at AS "created_at!"
          FROM eval_cases
          WHERE ($1::bool IS NOT TRUE OR enabled)
          ORDER BY created_at DESC"#,
        enabled_only,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

// Why: lint-ok: unused-pub — consumed by the evals dashboard page currently in
// development.
pub async fn delete_case(pool: &PgPool, case_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query!("DELETE FROM eval_cases WHERE id = $1", case_id)
        .execute(pool)
        .await?;
    Ok(())
}
