//! The three `proposal_*` handlers; their typed outputs live in
//! [`super::proposal_outputs`].
//!
//! `proposal_decide` checks the caller's Odoo link *before* it resolves the
//! approval row: resolving first would consume the approval and leave the
//! document stuck on a credential the approver does not have.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::policy::{ApprovalRepository, ApprovalStatus, ApprovalVerdict};
use systemprompt_mcp_odoo::client::OdooConnection;
use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::identity::resolve_credentials;
use uuid::Uuid;

pub use super::proposal_outputs::{
    FeedRow, ProposalDecideOutput, ProposalGetOutput, ProposalListOutput, ViewerCapability,
};
use crate::proposal::DocumentStatus;
use crate::proposal::apply::AppliedOutcome;
use crate::proposal::body::{BodySource, chatter_body};
use crate::proposal::plan::validate_selection;
use crate::proposal::settle::{SettleOutcome, settle_document};
use crate::store::{FeedFilter, KnowledgeStore};
use crate::tools::{
    DecisionInput, ProposalDecideInput, ProposalGetInput, ProposalListInput, TOOL_PROPOSAL_DECIDE,
    TOOL_PROPOSAL_GET, TOOL_PROPOSAL_LIST,
};

fn internal(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}

fn parse_document_id(raw: &str) -> Result<Uuid, McpError> {
    Uuid::parse_str(raw.trim())
        .map_err(|e| McpError::invalid_params(format!("document_id is not a UUID: {e}"), None))
}

async fn viewer_capability(store: &KnowledgeStore, ctx: &SysRequestContext) -> ViewerCapability {
    let (can_apply, odoo_login) = match resolve_credentials(store.pool(), ctx.user_id()).await {
        Ok(creds) => (true, Some(creds.login)),
        Err(OdooError::NotLinked(_)) => (false, None),
        Err(e) => {
            tracing::warn!(error = %e, "could not resolve the viewer's Odoo credential");
            (false, None)
        },
    };
    ViewerCapability {
        can_apply,
        odoo_login,
        link_url: "/admin/profile".to_owned(),
    }
}

pub(super) struct ProposalListHandler {
    pub(super) store: KnowledgeStore,
}

impl McpToolHandler for ProposalListHandler {
    type Input = ProposalListInput;
    type Output = ProposalListOutput;

    fn tool_name(&self) -> &'static str {
        TOOL_PROPOSAL_LIST
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let status = input
            .status
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty() && *s != "all")
            .map(|s| {
                DocumentStatus::parse(s).ok_or_else(|| {
                    McpError::invalid_params(format!("unknown status \"{s}\""), None)
                })
            })
            .transpose()?;
        let filter = FeedFilter {
            status,
            query: input.query,
            limit: input.limit.map(i64::from),
        };
        let docs = self.store.list_feed(&filter).await.map_err(internal)?;
        let rows: Vec<FeedRow> = docs.iter().map(FeedRow::from).collect();
        let proposed = rows
            .iter()
            .filter(|r| r.status == DocumentStatus::Proposed)
            .count();
        let summary = format!(
            "{} captured email(s), {proposed} awaiting approval",
            rows.len()
        );
        Ok((
            ProposalListOutput {
                viewer: viewer_capability(&self.store, ctx).await,
                odoo_web_base: OdooConnection::from_env().ok().map(|c| c.url),
                rows,
            },
            summary,
        ))
    }
}

pub(super) struct ProposalGetHandler {
    pub(super) store: KnowledgeStore,
}

impl McpToolHandler for ProposalGetHandler {
    type Input = ProposalGetInput;
    type Output = ProposalGetOutput;

    fn tool_name(&self) -> &'static str {
        TOOL_PROPOSAL_GET
    }

    async fn handle(
        &self,
        input: Self::Input,
        _ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let id = parse_document_id(&input.document_id)?;
        let doc = self
            .store
            .find_proposal_document(id)
            .await
            .map_err(internal)?
            .ok_or_else(|| McpError::invalid_params(format!("no document {id}"), None))?;
        let rfc5322_id = doc.rfc5322_id();
        let received = doc.received().unwrap_or_default();
        let document_id = doc.id.to_string();
        let sender = doc
            .proposal
            .as_ref()
            .map(|p| p.sender.clone())
            .or_else(|| {
                doc.metadata
                    .from
                    .as_deref()
                    .and_then(crate::proposal::sender::parse_mailbox)
            })
            .unwrap_or_else(|| crate::proposal::Sender {
                name: None,
                email: "unknown".to_owned(),
            });
        let body_html = chatter_body(&BodySource {
            sender: &sender,
            subject: &doc.title,
            received: &received,
            rfc5322_id: &rfc5322_id,
            content: &doc.content,
            document_id: &document_id,
        });
        let summary = format!("{} — {}", doc.title, doc.status.as_str());
        Ok((
            ProposalGetOutput {
                row: FeedRow::from(&doc),
                body_html,
            },
            summary,
        ))
    }
}

pub(super) struct ProposalDecideHandler {
    pub(super) store: KnowledgeStore,
}

impl McpToolHandler for ProposalDecideHandler {
    type Input = ProposalDecideInput;
    type Output = ProposalDecideOutput;

    fn tool_name(&self) -> &'static str {
        TOOL_PROPOSAL_DECIDE
    }

    async fn handle(
        &self,
        input: Self::Input,
        ctx: &SysRequestContext,
        _exec_id: &McpExecutionId,
    ) -> Result<(Self::Output, String), McpError> {
        let id = parse_document_id(&input.document_id)?;
        let doc = self
            .store
            .find_proposal_document(id)
            .await
            .map_err(internal)?
            .ok_or_else(|| McpError::invalid_params(format!("no document {id}"), None))?;
        let call_id = doc.proposal_call_id.clone().ok_or_else(|| {
            McpError::invalid_params(
                format!(
                    "document {id} is {}; nothing to decide",
                    doc.status.as_str()
                ),
                None,
            )
        })?;

        if input.decision == DecisionInput::Approve {
            resolve_credentials(self.store.pool(), ctx.user_id())
                .await
                .map_err(McpError::from)?;
            if let Some(proposal) = &doc.proposal {
                validate_selection(&proposal.actions, &input.exclude_actions)
                    .map_err(|e| McpError::invalid_params(e.to_string(), None))?;
            }
        }

        let pg = self.store.write_pool().map_err(internal)?;
        let repo = ApprovalRepository::new((*pg).clone());
        let status = match input.decision {
            DecisionInput::Approve => ApprovalStatus::Approved,
            DecisionInput::Reject => ApprovalStatus::Denied,
        };
        let username = ctx
            .user
            .as_ref()
            .map_or_else(|| ctx.user_id().to_string(), |u| u.username.clone());
        let note = decision_note(input.note.as_deref(), &input.exclude_actions);
        // Why: a lost race here is not an error — the admin page may have
        // answered first — so the row is re-read and settled either way.
        repo.resolve(
            &call_id,
            &ApprovalVerdict {
                status,
                approver_id: ctx.user_id(),
                approver_username: &username,
                note: note.as_deref(),
            },
        )
        .await
        .map_err(internal)?;
        let request = repo
            .find(&call_id)
            .await
            .map_err(internal)?
            .ok_or_else(|| {
                McpError::invalid_request("the approval row has vanished".to_owned(), None)
            })?;

        let outcome = settle_document(&self.store, id, &request, &input.exclude_actions)
            .await
            .map_err(internal)?;
        let (status, applied, message) = describe(outcome);
        Ok((
            ProposalDecideOutput {
                document_id: id.to_string(),
                status,
                applied,
                message: message.clone(),
            },
            message,
        ))
    }
}

fn describe(outcome: SettleOutcome) -> (DocumentStatus, Option<AppliedOutcome>, String) {
    match outcome {
        SettleOutcome::Applied(applied) => {
            let n = applied.actions.len();
            (
                DocumentStatus::Applied,
                Some(applied),
                format!("Applied {n} action(s) to Odoo"),
            )
        },
        SettleOutcome::Failed(error) => (
            DocumentStatus::Failed,
            None,
            format!("Approved, but applying failed and will be retried: {error}"),
        ),
        SettleOutcome::Denied => (DocumentStatus::Denied, None, "Rejected".to_owned()),
        SettleOutcome::Expired => (DocumentStatus::Expired, None, "Expired".to_owned()),
        SettleOutcome::NotPending(status) => (
            status,
            None,
            format!("Already {}; nothing changed", status.as_str()),
        ),
    }
}

fn decision_note(note: Option<&str>, exclude: &[usize]) -> Option<String> {
    let note = note.map(str::trim).filter(|n| !n.is_empty());
    match (note, exclude.is_empty()) {
        (None, true) => None,
        (Some(n), true) => Some(n.to_owned()),
        (None, false) => Some(format!("excluded actions {exclude:?}")),
        (Some(n), false) => Some(format!("{n} (excluded actions {exclude:?})")),
    }
}
