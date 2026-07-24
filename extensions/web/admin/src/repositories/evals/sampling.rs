//! Candidate selection for a judge run.
//!
//! Two rules make the sample honest:
//!
//! 1. `actor_kind <> 'job'` — judge calls are themselves gateway requests, so
//!    without this the next run would grade the previous run's judgements and
//!    the population would drift towards our own output.
//! 2. Round-robin over models — a run that sampled purely by recency would be
//!    dominated by whichever model the user happened to be using that hour,
//!    and per-model scores would not be comparable.

use serde::Serialize;
use sqlx::PgPool;
use systemprompt::identifiers::{AiRequestId, SessionId, UserId};

use crate::util::time_range::TimeRange;

/// Which slice of traffic a run should draw from. Mirrors the page filters.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CandidateFilter {
    pub user_id: Option<UserId>,
    pub model: Option<String>,
    pub provider: Option<String>,
    /// Only sample requests that have not yet been scored by this judge model.
    pub skip_judged_by: Option<String>,
}

/// One request plus the stored bodies the judge needs to read.
#[derive(Debug, Clone)]
pub struct EvalCandidate {
    pub ai_request_id: AiRequestId,
    pub user_id: UserId,
    pub session_id: Option<SessionId>,
    pub provider: String,
    pub model: String,
    pub status: String,
    pub error_message: Option<String>,
    pub latency_ms: Option<i32>,
    pub cost_microdollars: i64,
    pub request_body: Option<serde_json::Value>,
    pub response_body: Option<serde_json::Value>,
    pub request_excerpt: Option<String>,
    pub response_excerpt: Option<String>,
    pub response_truncated: bool,
}

pub async fn list_eval_candidates(
    pool: &PgPool,
    filter: &CandidateFilter,
    range: TimeRange,
    limit: i64,
) -> Result<Vec<EvalCandidate>, sqlx::Error> {
    let rows = sqlx::query!(
        r#"WITH eligible AS (
            SELECT
                ar.id,
                ar.user_id,
                ar.session_id,
                ar.provider,
                ar.model,
                ar.status,
                ar.error_message,
                ar.latency_ms,
                COALESCE(ar.cost_microdollars, 0)::bigint AS cost_microdollars,
                ar.created_at,
                p.request_body,
                p.response_body,
                p.request_excerpt,
                p.response_excerpt,
                p.response_truncated,
                ROW_NUMBER() OVER (PARTITION BY ar.model ORDER BY ar.created_at DESC) AS model_rank
            FROM ai_requests ar
            JOIN ai_request_payloads p ON p.ai_request_id = ar.id
            WHERE ar.created_at >= $1 AND ar.created_at < $2
              AND ar.actor_kind <> 'job'
              AND ($3::text IS NULL OR ar.user_id = $3)
              AND ($4::text IS NULL OR ar.model = $4)
              AND ($5::text IS NULL OR ar.provider = $5)
              AND ($6::text IS NULL OR NOT EXISTS (
                  SELECT 1
                  FROM eval_results er
                  JOIN eval_runs run ON run.id = er.run_id
                  WHERE er.ai_request_id = ar.id AND run.judge_model = $6
              ))
        )
        SELECT
            id AS "id!: AiRequestId",
            user_id AS "user_id!: UserId",
            session_id AS "session_id: SessionId",
            provider AS "provider!",
            model AS "model!",
            status AS "status!",
            error_message,
            latency_ms,
            cost_microdollars AS "cost_microdollars!",
            request_body,
            response_body,
            request_excerpt,
            response_excerpt,
            response_truncated AS "response_truncated!"
        FROM eligible
        ORDER BY model_rank, created_at DESC
        LIMIT $7"#,
        range.from,
        range.to,
        filter.user_id.as_ref().map(UserId::as_str),
        filter.model.as_deref(),
        filter.provider.as_deref(),
        filter.skip_judged_by.as_deref(),
        limit,
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| EvalCandidate {
            ai_request_id: r.id,
            user_id: r.user_id,
            session_id: r.session_id,
            provider: r.provider,
            model: r.model,
            status: r.status,
            error_message: r.error_message,
            latency_ms: r.latency_ms,
            cost_microdollars: r.cost_microdollars,
            request_body: r.request_body,
            response_body: r.response_body,
            request_excerpt: r.request_excerpt,
            response_excerpt: r.response_excerpt,
            response_truncated: r.response_truncated,
        })
        .collect())
}

/// Everything needed to freeze one request into the golden set.
pub async fn find_candidate_by_id(
    pool: &PgPool,
    ai_request_id: &str,
) -> Result<Option<EvalCandidate>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT
            ar.id AS "id!: AiRequestId",
            ar.user_id AS "user_id!: UserId",
            ar.session_id AS "session_id: SessionId",
            ar.provider AS "provider!",
            ar.model AS "model!",
            ar.status AS "status!",
            ar.error_message,
            ar.latency_ms,
            COALESCE(ar.cost_microdollars, 0)::bigint AS "cost_microdollars!",
            p.request_body,
            p.response_body,
            p.request_excerpt,
            p.response_excerpt,
            p.response_truncated AS "response_truncated!"
          FROM ai_requests ar
          JOIN ai_request_payloads p ON p.ai_request_id = ar.id
          WHERE ar.id = $1 OR ar.request_id = $1
          LIMIT 1"#,
        ai_request_id,
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| EvalCandidate {
        ai_request_id: r.id,
        user_id: r.user_id,
        session_id: r.session_id,
        provider: r.provider,
        model: r.model,
        status: r.status,
        error_message: r.error_message,
        latency_ms: r.latency_ms,
        cost_microdollars: r.cost_microdollars,
        request_body: r.request_body,
        response_body: r.response_body,
        request_excerpt: r.request_excerpt,
        response_excerpt: r.response_excerpt,
        response_truncated: r.response_truncated,
    }))
}
