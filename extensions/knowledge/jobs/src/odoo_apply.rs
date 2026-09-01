//! `knowledge_odoo_apply` job: the reconcile loop behind the approval queue.
//!
//! An approval answered on `/admin/governance/approvals` has nobody waiting on
//! it — the page only writes the row. This job finds every proposal whose row
//! has been answered and settles it through the same executor the
//! `proposal_decide` tool uses, sweeps expired rows, and retries applies that
//! failed (an approver without an Odoo link, Odoo down) with backoff.

use std::sync::Arc;

use systemprompt::database::DbPool;
use systemprompt::security::policy::ApprovalRepository;
use systemprompt::traits::{Job, JobContext, JobResult};
use systemprompt_mcp_knowledge_bank::proposal::settle::settle_document;
use systemprompt_mcp_knowledge_bank::store::{KnowledgeStore, SettleableRow};

use crate::error::KnowledgeJobError;

const DEFAULT_BATCH_SIZE: i64 = 50;

#[derive(Debug, Clone, Copy, Default)]
pub struct KnowledgeOdooApplyJob;

#[async_trait::async_trait]
impl Job for KnowledgeOdooApplyJob {
    fn name(&self) -> &'static str {
        "knowledge_odoo_apply"
    }

    fn description(&self) -> &'static str {
        "Settles answered ingestion approvals — applies to Odoo as the approver, records \
         denials and expiries, retries failed applies (parameter: batch_size)"
    }

    fn schedule(&self) -> &'static str {
        "0 * * * * *"
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
        let store = KnowledgeStore::new(Arc::clone(db));
        let pg = store.write_pool().map_err(KnowledgeJobError::from)?;
        let repo = ApprovalRepository::new((*pg).clone());
        let batch_size = ctx
            .get_parameter_parsed::<i64>("batch_size")?
            .unwrap_or(DEFAULT_BATCH_SIZE)
            .clamp(1, 500);

        // Why: nothing else sweeps 7-day proposal rows; the admin page only
        // expires on load, and it may not be loaded for days.
        match repo.expire_due().await {
            Ok(n) if n > 0 => {
                tracing::info!(expired = n, "knowledge_odoo_apply: swept expired approvals");
            },
            Ok(_) => {},
            Err(e) => tracing::warn!(error = %e, "knowledge_odoo_apply: expiry sweep failed"),
        }

        let mut due = store
            .list_settleable(batch_size)
            .await
            .map_err(KnowledgeJobError::from)?;
        due.extend(
            store
                .list_retry_due(batch_size)
                .await
                .map_err(KnowledgeJobError::from)?,
        );
        if due.is_empty() {
            return Ok(JobResult::success().with_stats(0, 0));
        }

        let mut success = 0u64;
        let mut failed = 0u64;
        for row in due {
            match settle_one(&store, &repo, &row).await {
                Ok(()) => success += 1,
                Err(e) => {
                    failed += 1;
                    tracing::warn!(document_id = %row.document_id, error = %e, "knowledge_odoo_apply: settle failed");
                },
            }
        }

        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(
            success,
            failed,
            duration_ms,
            "knowledge_odoo_apply: run complete"
        );
        Ok(JobResult::success()
            .with_stats(success, failed)
            .with_duration(duration_ms))
    }
}

async fn settle_one(
    store: &KnowledgeStore,
    repo: &ApprovalRepository,
    row: &SettleableRow,
) -> Result<(), KnowledgeJobError> {
    let request = repo.find(&row.call_id).await?.ok_or_else(|| {
        KnowledgeJobError::Other(format!("approval {} has vanished", row.call_id))
    })?;
    let outcome = settle_document(store, row.document_id, &request, &[]).await?;
    tracing::info!(document_id = %row.document_id, outcome = ?outcome, "knowledge_odoo_apply: settled");
    Ok(())
}

systemprompt::traits::submit_job!(&KnowledgeOdooApplyJob);
