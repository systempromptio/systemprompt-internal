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
use systemprompt::models::ai::{AiMessage, AiRequest, ResponseFormat, StructuredOutputOptions};
use systemprompt::system::AppContext;
use systemprompt::traits::{Job, JobContext, JobResult};
use uuid::Uuid;

use crate::ai::build_ai_service;
use crate::categorize_output::{
    parse_output, response_schema, structured_json, system_prompt, user_prompt,
};
use crate::error::KnowledgeJobError;

const DEFAULT_BATCH_SIZE: i64 = 10;
const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 1024;
const AGENT: &str = "knowledge-categorizer";

struct RawDocument {
    id: Uuid,
    title: String,
    content: String,
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
                    tracing::warn!(
                        document_id = %document.id,
                        error = %e,
                        "knowledge_categorization: left raw for retry"
                    );
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
        SELECT id, title, content
        FROM knowledge_documents
        WHERE status = 'raw'
        ORDER BY created_at
        LIMIT $1
        "#,
        limit,
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
    .with_structured_output(StructuredOutputOptions {
        response_format: Some(ResponseFormat::json_schema(response_schema())),
        ..StructuredOutputOptions::default()
    })
    .build();

    let response = run
        .ai
        .generate(&request)
        .await
        .map_err(|e| KnowledgeJobError::Other(format!("ai generate: {e}")))?;

    let categorization = parse_output(&response.content).ok_or_else(|| {
        KnowledgeJobError::Other(format!(
            "unparseable model output ({} chars)",
            response.content.len()
        ))
    })?;

    let structured = structured_json(&categorization);
    sqlx::query!(
        r#"
        UPDATE knowledge_documents
        SET category = $1, structured = $2, status = 'categorized'
        WHERE id = $3 AND status = 'raw'
        "#,
        categorization.category,
        structured,
        document.id,
    )
    .execute(run.pool)
    .await?;

    Ok(categorization.category)
}

systemprompt::traits::submit_job!(&KnowledgeCategorizationJob);
