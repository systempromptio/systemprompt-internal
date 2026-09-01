//! `email_ingestion` job: polls the brain@ mailbox over IMAP and captures
//! every unseen message into the knowledge bank as a raw document.
//!
//! Flow per run: fetch UNSEEN with `BODY.PEEK[]` (so a crash before commit
//! leaves the mailbox untouched), insert each message behind the Message-ID
//! ledger, then mark the successfully captured UIDs `\Seen`. The ledger —
//! not the flag — is the dedupe boundary, so a reset flag only costs a
//! refetch, never a duplicate row.

use sqlx::PgPool;
use systemprompt::database::DbPool;
use systemprompt::traits::{Job, JobContext, JobResult};

use crate::error::KnowledgeJobError;
use crate::imap_client::{ImapConfig, fetch_unseen, mark_seen};
use crate::mail::{CapturedEmail, captured_from_rfc822, metadata_json, render_document};

const DEFAULT_IMAP_HOST: &str = "imap.gmail.com";
const DEFAULT_IMAP_PORT: u16 = 993;
const DEFAULT_IMAP_USER: &str = "brain@systemprompt.io";
const DEFAULT_MAILBOX: &str = "INBOX";
const DEFAULT_MAX_BATCH: usize = 50;
const PASSWORD_ENV: &str = "BRAIN_IMAP_PASSWORD";
const PASSWORD_SECRET: &str = "brain_imap_password";

#[derive(Debug, Clone, Copy, Default)]
pub struct EmailIngestionJob;

#[async_trait::async_trait]
impl Job for EmailIngestionJob {
    fn name(&self) -> &'static str {
        "email_ingestion"
    }

    fn description(&self) -> &'static str {
        "Captures unseen brain@ mailbox emails into the knowledge bank as raw documents"
    }

    fn schedule(&self) -> &'static str {
        "0 */5 * * * *"
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

        let config = load_config(ctx)?;

        ensure_schema(db, &pool).await?;

        let fetch_config = config.clone();
        let fetched = tokio::task::spawn_blocking(move || fetch_unseen(&fetch_config))
            .await
            .map_err(|e| KnowledgeJobError::Other(format!("fetch task panicked: {e}")))??;

        if fetched.is_empty() {
            tracing::info!("email_ingestion: no unseen messages");
            return Ok(JobResult::success().with_stats(0, 0));
        }

        let uploaded_by = ctx.actor().user_id.as_str().to_owned();
        let mut captured_uids: Vec<u32> = Vec::new();
        let mut success = 0u64;
        let mut failed = 0u64;

        for message in &fetched {
            let fallback_id = format!("imap:{}:{}:{}", config.user, config.mailbox, message.uid);
            let Some(email) = captured_from_rfc822(&message.raw, &fallback_id) else {
                tracing::warn!(uid = message.uid, "email_ingestion: unparseable message");
                failed += 1;
                continue;
            };
            match ingest_one(&pool, &email, &uploaded_by).await {
                Ok(inserted) => {
                    success += 1;
                    captured_uids.push(message.uid);
                    tracing::info!(
                        uid = message.uid,
                        message_id = %email.mime_message_id,
                        subject = %email.subject,
                        inserted,
                        "email_ingestion: captured"
                    );
                },
                Err(e) => {
                    failed += 1;
                    tracing::warn!(
                        uid = message.uid,
                        message_id = %email.mime_message_id,
                        error = %e,
                        "email_ingestion: capture failed"
                    );
                },
            }
        }

        if !captured_uids.is_empty() {
            let seen_config = config.clone();
            let uids = captured_uids.clone();
            // Why: a failed \Seen store is non-fatal — the ledger already
            // guarantees these messages can never be ingested twice.
            if let Err(e) = tokio::task::spawn_blocking(move || mark_seen(&seen_config, &uids))
                .await
                .map_err(|e| KnowledgeJobError::Other(format!("seen task panicked: {e}")))?
            {
                tracing::warn!(error = %e, "email_ingestion: failed to mark messages seen");
            }
        }

        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(
            fetched = fetched.len(),
            success,
            failed,
            duration_ms,
            "email_ingestion: run complete"
        );

        Ok(JobResult::success()
            .with_stats(success, failed)
            .with_duration(duration_ms))
    }
}

fn load_config(ctx: &JobContext) -> Result<ImapConfig, KnowledgeJobError> {
    let password = std::env::var(PASSWORD_ENV).ok().or_else(|| {
        systemprompt::config::SecretsBootstrap::get()
            .ok()
            .and_then(|secrets| secrets.get(PASSWORD_SECRET).cloned())
    });
    let Some(password) = password else {
        return Err(KnowledgeJobError::Config(format!(
            "no IMAP password: set {PASSWORD_ENV} or the {PASSWORD_SECRET} secret"
        )));
    };

    let port = ctx
        .get_parameter("imap_port")
        .map(|p| p.parse::<u16>())
        .transpose()
        .map_err(|e| KnowledgeJobError::Config(format!("invalid imap_port: {e}")))?
        .unwrap_or(DEFAULT_IMAP_PORT);
    let max_batch = ctx
        .get_parameter("max_batch")
        .map(|p| p.parse::<usize>())
        .transpose()
        .map_err(|e| KnowledgeJobError::Config(format!("invalid max_batch: {e}")))?
        .unwrap_or(DEFAULT_MAX_BATCH);

    Ok(ImapConfig {
        host: ctx
            .get_parameter("imap_host")
            .cloned()
            .unwrap_or_else(|| DEFAULT_IMAP_HOST.to_owned()),
        port,
        user: ctx
            .get_parameter("imap_user")
            .cloned()
            .unwrap_or_else(|| DEFAULT_IMAP_USER.to_owned()),
        password,
        mailbox: ctx
            .get_parameter("mailbox")
            .cloned()
            .unwrap_or_else(|| DEFAULT_MAILBOX.to_owned()),
        max_batch,
    })
}

async fn ensure_schema(db: &DbPool, pool: &PgPool) -> Result<(), KnowledgeJobError> {
    // Why: the documents table belongs to the knowledge-bank crate and this
    // job may run before that extension's schema has been installed.
    systemprompt_mcp_knowledge_bank::schema::ensure_installed(db)
        .await
        .map_err(|e| KnowledgeJobError::Other(e.to_string()))?;
    sqlx::raw_sql(crate::extension::SCHEMA_EMAIL_INGEST)
        .execute(pool)
        .await?;
    sqlx::raw_sql(crate::extension::MIGRATION_CATEGORIZATION)
        .execute(pool)
        .await?;
    sqlx::raw_sql(crate::extension::MIGRATION_PROPOSAL)
        .execute(pool)
        .await?;
    Ok(())
}

// Why: the ledger row is claimed first so two concurrent runs can never both
// insert a document for the same Message-ID; `false` means already ingested.
async fn ingest_one(
    pool: &PgPool,
    email: &CapturedEmail,
    uploaded_by: &str,
) -> Result<bool, KnowledgeJobError> {
    let claimed = sqlx::query!(
        "INSERT INTO knowledge_email_ingest (message_id) VALUES ($1) ON CONFLICT DO NOTHING",
        email.mime_message_id,
    )
    .execute(pool)
    .await?
    .rows_affected();

    if claimed == 0 {
        return Ok(false);
    }

    let content = render_document(email);
    let metadata = metadata_json(email);

    let document_id = sqlx::query_scalar!(
        r"
        INSERT INTO knowledge_documents (title, source, project, content, uploaded_by, metadata, status)
        VALUES ($1, 'email', NULL, $2, $3, $4, 'raw')
        RETURNING id
        ",
        email.subject,
        content,
        uploaded_by,
        metadata,
    )
    .fetch_one(pool)
    .await?;

    sqlx::query!(
        "UPDATE knowledge_email_ingest SET document_id = $1 WHERE message_id = $2",
        document_id,
        email.mime_message_id,
    )
    .execute(pool)
    .await?;

    Ok(true)
}

systemprompt::traits::submit_job!(&EmailIngestionJob);
