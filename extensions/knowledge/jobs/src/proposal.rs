//! `knowledge_proposal` job: turns each categorized brain@ email into a
//! proposed Odoo projection and opens the approval a human must answer.
//!
//! Read-only against Odoo, as the job owner's linked account: it asks whether
//! the sender is already a partner or an open lead and whether Project is
//! installed, then runs the pure planner. The result is stored on the
//! document and mirrored into `approval_requests`; nothing is written to Odoo
//! here or anywhere else until that row says `approved`.

use std::sync::Arc;

use chrono::Utc;
use systemprompt::database::DbPool;
use systemprompt::identifiers::UserId;
use systemprompt::traits::{Job, JobContext, JobResult};
use systemprompt_mcp_knowledge_bank::proposal::approval::{open_proposal_hold, proposal_call_id};
use systemprompt_mcp_knowledge_bank::proposal::body::{BodySource, chatter_body};
use systemprompt_mcp_knowledge_bank::proposal::lookup::{
    OdooCapabilities, capabilities, lookup_sender,
};
use systemprompt_mcp_knowledge_bank::proposal::plan::{PlanInput, PlanOutcome, plan};
use systemprompt_mcp_knowledge_bank::proposal::scan::{ScanVerdict, scan_body};
use systemprompt_mcp_knowledge_bank::proposal::sender::parse_mailbox;
use systemprompt_mcp_knowledge_bank::proposal::{OdooAction, Proposal};
use systemprompt_mcp_knowledge_bank::store::{KnowledgeStore, ProposalDocument};
use systemprompt_mcp_odoo::client::{Credentials, OdooClient};
use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::identity::resolve_credentials;

use crate::error::KnowledgeJobError;

const DEFAULT_BATCH_SIZE: i64 = 10;
const SKIP_NO_SENDER: &str = "no_sender";
const SKIP_NO_INTENT: &str = "no_intent";
const SKIP_SECRET_SCAN: &str = "secret_scan_withheld";

#[derive(Debug, Clone, Copy, Default)]
pub struct KnowledgeProposalJob;

struct Run<'a> {
    store: &'a KnowledgeStore,
    client: &'a OdooClient,
    creds: &'a Credentials,
    owner: &'a UserId,
    capabilities: OdooCapabilities,
    task_project: Option<&'a str>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Proposed {
    Opened,
    Skipped,
}

#[async_trait::async_trait]
impl Job for KnowledgeProposalJob {
    fn name(&self) -> &'static str {
        "knowledge_proposal"
    }

    fn description(&self) -> &'static str {
        "Plans an Odoo projection for each categorized brain@ email and opens its human \
         approval (parameters: batch_size, task_project)"
    }

    fn schedule(&self) -> &'static str {
        "0 25 * * * *"
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
        let batch_size = ctx
            .get_parameter_parsed::<i64>("batch_size")?
            .unwrap_or(DEFAULT_BATCH_SIZE)
            .clamp(1, 100);
        let task_project = ctx
            .get_parameter("task_project")
            .map(|p| p.trim().to_owned())
            .filter(|p| !p.is_empty());

        let documents = store
            .list_categorized(batch_size)
            .await
            .map_err(KnowledgeJobError::from)?;
        if documents.is_empty() {
            tracing::info!("knowledge_proposal: nothing categorized to propose");
            return Ok(JobResult::success().with_stats(0, 0));
        }

        let owner = ctx.actor().user_id.clone();
        let creds = match resolve_credentials(db, &owner).await {
            Ok(creds) => creds,
            Err(OdooError::NotLinked(_)) => {
                return Err(KnowledgeJobError::Config(format!(
                    "the job owner ({}) has no linked Odoo account; link one on /admin/profile so \
                     proposals can look up existing partners and leads",
                    owner.as_str()
                ))
                .into());
            },
            Err(e) => return Err(KnowledgeJobError::Odoo(e).into()),
        };
        let client = OdooClient::from_env().map_err(KnowledgeJobError::Odoo)?;
        let capabilities = capabilities(&client, &creds)
            .await
            .map_err(KnowledgeJobError::Odoo)?;
        let run = Run {
            store: &store,
            client: &client,
            creds: &creds,
            owner: &owner,
            capabilities,
            task_project: task_project.as_deref(),
        };

        let mut success = 0u64;
        let mut failed = 0u64;
        for document in documents {
            match propose_one(&run, &document).await {
                Ok(outcome) => {
                    success += 1;
                    tracing::info!(
                        document_id = %document.id,
                        title = %document.title,
                        outcome = ?outcome,
                        "knowledge_proposal: planned"
                    );
                },
                Err(e) => {
                    failed += 1;
                    tracing::warn!(
                        document_id = %document.id,
                        error = %e,
                        "knowledge_proposal: left categorized for retry"
                    );
                },
            }
        }

        let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
        tracing::info!(
            success,
            failed,
            duration_ms,
            "knowledge_proposal: run complete"
        );
        Ok(JobResult::success()
            .with_stats(success, failed)
            .with_duration(duration_ms))
    }
}

async fn propose_one(run: &Run<'_>, doc: &ProposalDocument) -> Result<Proposed, KnowledgeJobError> {
    let Some(intent) = doc.structured.as_ref().and_then(|s| s.crm_intent.as_ref()) else {
        run.store.set_skipped(doc.id, SKIP_NO_INTENT).await?;
        return Ok(Proposed::Skipped);
    };
    let Some(sender) = doc.metadata.from.as_deref().and_then(parse_mailbox) else {
        run.store.set_skipped(doc.id, SKIP_NO_SENDER).await?;
        return Ok(Proposed::Skipped);
    };

    let lookup = lookup_sender(run.client, run.creds, &sender.email).await?;
    let outcome = plan(&PlanInput {
        category: doc.category.as_deref().unwrap_or("other"),
        subject: &doc.title,
        intent,
        sender: &sender,
        lookup: &lookup,
        capabilities: run.capabilities,
        task_project: run.task_project,
        today: Utc::now().date_naive(),
    });
    let actions = match outcome {
        PlanOutcome::Skip(reason) => {
            run.store.set_skipped(doc.id, reason.as_str()).await?;
            return Ok(Proposed::Skipped);
        },
        PlanOutcome::Propose(actions) => actions,
    };

    let actions = match scan_body(run.owner, &proposed_body(doc, &sender)) {
        ScanVerdict::Clean => actions,
        ScanVerdict::Withheld(reason) => {
            tracing::warn!(document_id = %doc.id, reason = %reason, "knowledge_proposal: chatter body withheld by secret_scan");
            let kept: Vec<OdooAction> = actions
                .into_iter()
                .filter(|a| !matches!(a, OdooAction::PostChatter { .. }))
                .collect();
            if kept.is_empty() {
                run.store.set_skipped(doc.id, SKIP_SECRET_SCAN).await?;
                return Ok(Proposed::Skipped);
            }
            kept
        },
    };

    let proposal = Proposal {
        revision: doc.proposal_revision + 1,
        sender,
        actions,
    };
    let call_id = proposal_call_id(run.owner, doc.id, &proposal)
        .ok_or_else(|| KnowledgeJobError::Other("proposal did not serialize".to_owned()))?;
    if !run
        .store
        .set_proposed(doc.id, &proposal, call_id.as_str())
        .await?
    {
        return Ok(Proposed::Skipped);
    }
    let pg = run.store.write_pool()?;
    if let Err(e) = open_proposal_hold(pg.as_ref(), run.owner, doc.id, &proposal).await {
        run.store.revert_proposed(doc.id, call_id.as_str()).await?;
        return Err(e.into());
    }
    Ok(Proposed::Opened)
}

// Why: the scanner sees exactly the HTML that would be posted, not the raw
// capture, so a secret that only survives escaping is still caught.
fn proposed_body(
    doc: &ProposalDocument,
    sender: &systemprompt_mcp_knowledge_bank::proposal::Sender,
) -> String {
    let rfc5322_id = doc.rfc5322_id();
    let received = doc.received().unwrap_or_default();
    let document_id = doc.id.to_string();
    chatter_body(&BodySource {
        sender,
        subject: &doc.title,
        received: &received,
        rfc5322_id: &rfc5322_id,
        content: &doc.content,
        document_id: &document_id,
    })
}

systemprompt::traits::submit_job!(&KnowledgeProposalJob);
