//! `knowledge_categorization` job: the Phase 2 pass over captured knowledge.
//!
//! Selects `status='raw'` documents oldest-first, asks the AI gateway for a
//! category + structured summary + `crm_intent` (JSON-schema constrained),
//! and writes `category`/`structured` back, flipping `status` to
//! `categorized`. A document that fails stays `raw` and is retried on the next
//! run. This job never reads or writes Odoo and never creates CRM leads: what
//! the intent *becomes* is the `knowledge_proposal` job's decision, and every
//! Odoo write waits on a human.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt::ai::AiService;
use systemprompt::database::DbPool;
use systemprompt::identifiers::{Actor, AgentName, ContextId, SessionId, TraceId};
use systemprompt::models::RequestContext;
use systemprompt::models::ai::{AiMessage, AiRequest, AiResponse};
use systemprompt::system::AppContext;
use systemprompt::traits::{Job, JobContext, JobResult};
use uuid::Uuid;

use crate::ai::build_ai_service;
use crate::categorize_output::{
    correction_prompt, parse_output, structured_json, structured_output_options, system_prompt,
    user_prompt,
};
use crate::error::KnowledgeJobError;

const DEFAULT_BATCH_SIZE: i64 = 10;
// Why: a rich thread's summary, entities, tasks and intent do not fit in 1 KiB
// of tokens; a truncated object fails validation and burns the call.
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 4096;
// Why: after this many rejected responses the document is parked as skipped,
// visible in the feed with its last error, instead of being retried hourly.
const MAX_ATTEMPTS: i32 = 3;
const AGENT: &str = "knowledge-categorizer";

struct RawDocument {
    id: Uuid,
    title: String,
    content: String,
    attempts: i32,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct KnowledgeCategorizationJob;

#[async_trait::async_trait]
impl Job for KnowledgeCategorizationJob {
    fn name(&self) -> &'static str {
        "knowledge_categorization"
    }

    fn description(&self) -> &'static str {
        "Categorizes raw knowledge-bank documents into structured data via the AI gateway \
         (parameters: batch_size, provider, model, max_output_tokens)"
    }

    fn schedule(&self) -> &'static str {
        "0 15 * * * *"
    }

    fn tags(&self) -> Vec<&'static str> {
        vec![crate::registry::JOB_TAG]
    }

    async fn execute(
        &self,
        ctx: &JobContext,
    ) -> Result<JobResult, systemprompt::traits::ProviderError> {
        let start = std::time::Instant::now();

        let db = ctx
            .db_pool::<DbPool>()
            .ok_or(KnowledgeJobError::MissingContext("DbPool"))?;
        let pool = db
            .write_pool()
            .ok_or(KnowledgeJobError::MissingContext("write PgPool"))?;
        let app_context = ctx
            .app_context::<Arc<AppContext>>()
            .ok_or(KnowledgeJobError::MissingContext("AppContext"))?;

        let batch_size = ctx
            .get_parameter_parsed::<i64>("batch_size")?
            .unwrap_or(DEFAULT_BATCH_SIZE)
            .clamp(1, 100);
        let max_output_tokens = ctx
            .get_parameter_parsed::<u32>("max_output_tokens")?
            .unwrap_or(DEFAULT_MAX_OUTPUT_TOKENS);

        let documents = list_raw_documents(&pool, batch_size).await?;
        if documents.is_empty() {
            tracing::info!("knowledge_categorization: nothing raw to categorize");
            return Ok(JobResult::success().with_stats(0, 0));
        }

        let ai = build_ai_service(db, app_context)?;
        let run = CategorizeRun {
            pool: &pool,
            provider: ctx
                .get_parameter("provider")
                .cloned()
                .unwrap_or_else(|| ai.default_provider().to_owned()),
            model: ctx
                .get_parameter("model")
                .cloned()
                .unwrap_or_else(|| ai.default_model().to_owned()),
            max_output_tokens,
            actor: Actor::job(ctx.actor().user_id.clone(), self.name()),
            ai,
        };

        let mut success = 0u64;
        let mut failed = 0u64;
        for document in documents {
            match categorize_one(&run, &document).await {
                Ok(category) => {
                    success += 1;
                    tracing::info!(
                        document_id = %document.id,
                        title = %document.title,
                        category = %category,
                        "knowledge_categorization: categorized"
                    );
                },
                Err(e) => {
                    failed += 1;
                    let attempts = document.attempts + 1;
                    tracing::warn!(
                        document_id = %document.id,
                        attempts,
                        error = %e,
                        "knowledge_categorization: response rejected"
                    );
                    record_failure(&pool, document.id, attempts, &e.to_string()).await?;
                },
            }
        }

        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(
            success,
            failed,
            duration_ms,
            "knowledge_categorization: run complete"
        );
        Ok(JobResult::success()
            .with_stats(success, failed)
            .with_duration(duration_ms))
    }
}

async fn list_raw_documents(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<RawDocument>, KnowledgeJobError> {
    let rows = sqlx::query_as!(
        RawDocument,
        r#"
        SELECT id, title, content,
               COALESCE((metadata->>'categorization_attempts')::int, 0) AS "attempts!"
        FROM knowledge_documents
        WHERE status = 'raw'
          AND COALESCE((metadata->>'categorization_attempts')::int, 0) < $2
        ORDER BY created_at
        LIMIT $1
        "#,
        limit,
        MAX_ATTEMPTS,
    )
    .fetch_all(pool)
    .await?;
    Ok(rows)
}

struct CategorizeRun<'a> {
    pool: &'a PgPool,
    ai: Arc<AiService>,
    provider: String,
    model: String,
    max_output_tokens: u32,
    actor: Actor,
}

async fn categorize_one(
    run: &CategorizeRun<'_>,
    document: &RawDocument,
) -> Result<String, KnowledgeJobError> {
    let context = RequestContext::new(
        SessionId::generate(),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new(AGENT),
    )
    .with_actor(run.actor.clone());

    let request = AiRequest::builder(
        vec![AiMessage::user(user_prompt(
            &document.title,
            &document.content,
        ))],
        run.provider.clone(),
        run.model.clone(),
        run.max_output_tokens,
        context,
    )
    .with_system_prompt(system_prompt())
    .with_structured_output(structured_output_options())
    .build();

    let response = generate(run, &request).await?;

    // Why: one corrective round is cheap and fixes most rejections — the
    // validator's message names the exact path that broke, and the model is
    // shown its own answer next to it. A second rejection is recorded.
    let categorization = match parse_output(&response.content) {
        Ok(categorization) => categorization,
        Err(first_error) => {
            let retry = AiRequest::builder(
                vec![
                    AiMessage::user(user_prompt(&document.title, &document.content)),
                    AiMessage::assistant(response.content.clone()),
                    AiMessage::user(correction_prompt(&first_error)),
                ],
                run.provider.clone(),
                run.model.clone(),
                run.max_output_tokens,
                RequestContext::new(
                    SessionId::generate(),
                    TraceId::generate(),
                    ContextId::generate(),
                    AgentName::new(AGENT),
                )
                .with_actor(run.actor.clone()),
            )
            .with_system_prompt(system_prompt())
            .with_structured_output(structured_output_options())
            .build();
            let second = generate(run, &retry).await?;
            parse_output(&second.content).map_err(|second_error| {
                KnowledgeJobError::Other(format!(
                    "rejected twice — first: {first_error}; after correction: {second_error}"
                ))
            })?
        },
    };

    let structured = structured_json(&categorization);
    sqlx::query!(
        r#"
        UPDATE knowledge_documents
        SET category = $1, structured = $2, status = 'categorized'
        WHERE id = $3 AND status = 'raw'
        "#,
        categorization.category.as_str(),
        structured,
        document.id,
    )
    .execute(run.pool)
    .await?;

    Ok(categorization.category.as_str().to_owned())
}

async fn generate(
    run: &CategorizeRun<'_>,
    request: &AiRequest,
) -> Result<AiResponse, KnowledgeJobError> {
    run.ai
        .generate(request)
        .await
        .map_err(|e| KnowledgeJobError::Other(format!("ai generate: {e}")))
}

// Why: the attempt count and last error live on the document, so the feed
// shows why an email never became a proposal; at the cap it parks as skipped.
async fn record_failure(
    pool: &PgPool,
    id: Uuid,
    attempts: i32,
    error: &str,
) -> Result<(), KnowledgeJobError> {
    sqlx::query!(
        r#"
        UPDATE knowledge_documents
        SET metadata = COALESCE(metadata, '{}'::jsonb)
                || jsonb_build_object(
                    'categorization_attempts', $2::int,
                    'categorization_error', $3::text,
                    'categorization_failed_at', now()
                ),
            status = CASE WHEN $2 >= $4 THEN 'skipped' ELSE status END,
            skip_reason = CASE WHEN $2 >= $4 THEN 'categorization_failed' ELSE skip_reason END
        WHERE id = $1 AND status = 'raw'
        "#,
        id,
        attempts,
        error,
        MAX_ATTEMPTS,
    )
    .execute(pool)
    .await?;
    Ok(())
}

systemprompt::traits::submit_job!(&KnowledgeCategorizationJob);
