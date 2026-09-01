//! Opening the governance hold for a proposal.
//!
//! The row is the same `approval_requests` shape a held MCP call parks on,
//! with a synthetic tool name, so `/admin/governance/approvals` lists it with
//! no new plumbing. Nothing waits on it: the reconcile job and the
//! `proposal_decide` tool pick the answer up later.

use sqlx::PgPool;
use systemprompt::identifiers::{CallId, SessionId, UserId};
use systemprompt::security::policy::{ApprovalRepository, NewApprovalRequest};
use systemprompt_mcp_shared::approval::{
    DecisionPrincipal, DecisionSubject, derive_call_id, record_hold,
};
use uuid::Uuid;

use super::{
    PROPOSAL_EXPIRY_SECONDS, Proposal, ProposalArguments, RULE_BRAIN_EMAIL_INGEST,
    TOOL_APPLY_PROPOSAL,
};
use crate::error::KnowledgeBankError;
use crate::tools::SERVER_NAME;

#[must_use]
pub fn proposal_call_id(owner: &UserId, document_id: Uuid, proposal: &Proposal) -> Option<CallId> {
    let arguments = serde_json::to_value(ProposalArguments {
        document_id,
        revision: proposal.revision,
        proposal: proposal.clone(),
    })
    .ok()?;
    Some(derive_call_id(
        owner,
        SERVER_NAME,
        TOOL_APPLY_PROPOSAL,
        &arguments,
    ))
}

pub async fn open_proposal_hold(
    pool: &PgPool,
    owner: &UserId,
    document_id: Uuid,
    proposal: &Proposal,
) -> Result<CallId, KnowledgeBankError> {
    let arguments = serde_json::to_value(ProposalArguments {
        document_id,
        revision: proposal.revision,
        proposal: proposal.clone(),
    })?;
    let call_id = derive_call_id(owner, SERVER_NAME, TOOL_APPLY_PROPOSAL, &arguments);
    let trace_id = document_id.to_string();

    ApprovalRepository::new(pool.clone())
        .open(&NewApprovalRequest {
            call_id: &call_id,
            tool_name: TOOL_APPLY_PROPOSAL,
            server_name: SERVER_NAME,
            arguments: &arguments,
            requested_by: owner,
            session_id: None,
            trace_id: Some(&trace_id),
            rule: RULE_BRAIN_EMAIL_INGEST,
            expires_in_seconds: PROPOSAL_EXPIRY_SECONDS,
        })
        .await
        .map_err(|e| KnowledgeBankError::Internal(format!("could not open the approval: {e}")))?;

    let session_id = SessionId::generate();
    record_hold(
        pool,
        &DecisionSubject {
            call_id: &call_id,
            server_name: SERVER_NAME,
            tool_name: TOOL_APPLY_PROPOSAL,
        },
        &DecisionPrincipal {
            user_id: owner,
            session_id: &session_id,
            roles: &[],
            client_id: None,
            trace_id: Some(&trace_id),
            act_chain: &[],
        },
        RULE_BRAIN_EMAIL_INGEST,
    )
    .await;
    Ok(call_id)
}
