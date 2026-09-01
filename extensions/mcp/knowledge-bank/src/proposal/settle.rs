//! The single executor for a decided proposal.
//!
//! Both the `proposal_decide` tool and the reconcile job land here with the
//! `approval_requests` row that answers a proposal, and nothing else applies
//! one. The verdict is stamped into `governance_decisions` exactly once (on
//! the `proposed → approved|denied` transition), Odoo is written as the
//! approver's own credential, and every failure leaves the document in a
//! state the reconcile job can pick up again.

use chrono::Utc;
use systemprompt::identifiers::{SessionId, UserId};
use systemprompt::security::policy::{ApprovalRequest, ApprovalStatus};
use systemprompt_mcp_odoo::client::OdooClient;
use systemprompt_mcp_odoo::error::OdooError;
use systemprompt_mcp_odoo::identity::resolve_credentials;
use systemprompt_mcp_shared::approval::{
    DecisionPrincipal, DecisionSubject, approver_stamp, record_verdict,
};
use uuid::Uuid;

use super::apply::{AppliedOutcome, ApplyContext, ApplySource, apply_document};
use super::body::{BodySource, chatter_body};
use super::ledger::{LedgerKey, NewProjection, mark_excluded};
use super::{ActionTarget, DocumentStatus, OdooAction, TOOL_APPLY_PROPOSAL};
use crate::error::KnowledgeBankError;
use crate::store::{KnowledgeStore, ProposalDocument};
use crate::tools::SERVER_NAME;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SettleOutcome {
    Applied(AppliedOutcome),
    Failed(String),
    Denied,
    Expired,
    NotPending(DocumentStatus),
}

pub async fn settle_document(
    store: &KnowledgeStore,
    document_id: Uuid,
    request: &ApprovalRequest,
    exclude: &[usize],
) -> Result<SettleOutcome, KnowledgeBankError> {
    let doc = store
        .find_proposal_document(document_id)
        .await?
        .ok_or_else(|| KnowledgeBankError::NotFound(format!("document {document_id}")))?;
    if doc.proposal_call_id.as_deref() != Some(request.call_id.as_str()) {
        return Err(KnowledgeBankError::Invalid(format!(
            "approval {} does not belong to document {document_id}",
            request.call_id
        )));
    }

    let approver = UserId::new(request.approver_id.clone().unwrap_or_default());
    let decided_at = request.decided_at.unwrap_or_else(Utc::now);

    match request.status {
        ApprovalStatus::Pending => Ok(SettleOutcome::NotPending(doc.status)),
        ApprovalStatus::Denied => {
            if store
                .set_decided(document_id, DocumentStatus::Denied, &approver, decided_at)
                .await?
            {
                audit_verdict(store, &doc, request, false).await;
                Ok(SettleOutcome::Denied)
            } else {
                Ok(SettleOutcome::NotPending(doc.status))
            }
        },
        ApprovalStatus::Expired => {
            if store
                .set_decided(document_id, DocumentStatus::Expired, &approver, decided_at)
                .await?
            {
                Ok(SettleOutcome::Expired)
            } else {
                Ok(SettleOutcome::NotPending(doc.status))
            }
        },
        ApprovalStatus::Approved => approve(store, doc, request, exclude).await,
    }
}

async fn approve(
    store: &KnowledgeStore,
    doc: ProposalDocument,
    request: &ApprovalRequest,
    exclude: &[usize],
) -> Result<SettleOutcome, KnowledgeBankError> {
    let approver = UserId::new(request.approver_id.clone().unwrap_or_default());
    let decided_at = request.decided_at.unwrap_or_else(Utc::now);

    // Why: the first claim is the decision and is audited; a claim from
    // `failed` is a retry of the same decision and is not.
    if store
        .claim_for_apply(doc.id, DocumentStatus::Proposed, &approver, decided_at)
        .await?
    {
        audit_verdict(store, &doc, request, true).await;
    } else if !store
        .claim_for_apply(doc.id, DocumentStatus::Failed, &approver, decided_at)
        .await?
    {
        return Ok(SettleOutcome::NotPending(doc.status));
    }

    let Some(proposal) = doc.proposal.clone() else {
        let error = "document has no proposal to apply".to_owned();
        store.set_applied(doc.id, None, Some(&error)).await?;
        return Ok(SettleOutcome::Failed(error));
    };

    let creds = match resolve_credentials(store.pool(), &approver).await {
        Ok(creds) => creds,
        Err(OdooError::NotLinked(_)) => {
            let error = format!(
                "approver {} has no linked Odoo account; link one on /admin/profile and the \
                 proposal will be retried",
                request.approver_username.as_deref().unwrap_or("(unknown)")
            );
            store.set_applied(doc.id, None, Some(&error)).await?;
            return Ok(SettleOutcome::Failed(error));
        },
        Err(e) => {
            let error = e.to_string();
            store.set_applied(doc.id, None, Some(&error)).await?;
            return Ok(SettleOutcome::Failed(error));
        },
    };
    let client = match OdooClient::from_env() {
        Ok(client) => client,
        Err(e) => {
            let error = e.to_string();
            store.set_applied(doc.id, None, Some(&error)).await?;
            return Ok(SettleOutcome::Failed(error));
        },
    };

    let pg = store.write_pool()?;
    let rfc5322_id = doc.rfc5322_id();
    for &index in exclude {
        if let Some(action) = proposal.actions.get(index) {
            mark_excluded(
                pg.as_ref(),
                &NewProjection {
                    key: LedgerKey {
                        document_id: doc.id,
                        revision: proposal.revision,
                        action_index: i32::try_from(index).unwrap_or(i32::MAX),
                    },
                    kind: action.kind(),
                    res_model: action_model(action),
                    rfc5322_id: &rfc5322_id,
                    applied_by: approver.as_str(),
                    odoo_login: &creds.login,
                },
            )
            .await
            .map_err(|e| KnowledgeBankError::Internal(format!("ledger exclusion failed: {e}")))?;
        }
    }

    let received = doc.received().unwrap_or_default();
    let document_id = doc.id.to_string();
    let body_html = chatter_body(&BodySource {
        sender: &proposal.sender,
        subject: &doc.title,
        received: &received,
        rfc5322_id: &rfc5322_id,
        content: &doc.content,
        document_id: &document_id,
    });
    let outcome = apply_document(
        &ApplyContext {
            pool: pg.as_ref(),
            client: &client,
            creds: &creds,
            approver: &approver,
        },
        &ApplySource {
            document_id: doc.id,
            revision: proposal.revision,
            rfc5322_id: &rfc5322_id,
            email_from: &proposal.sender.email,
            subject: &doc.title,
            body_html: &body_html,
        },
        &proposal.actions,
    )
    .await;

    let error = (!outcome.all_ok).then(|| {
        outcome
            .actions
            .iter()
            .filter_map(|a| a.error.as_deref())
            .collect::<Vec<_>>()
            .join("; ")
    });
    store
        .set_applied(doc.id, Some(&outcome), error.as_deref())
        .await?;
    Ok(match error {
        Some(error) => SettleOutcome::Failed(error),
        None => SettleOutcome::Applied(outcome),
    })
}

fn action_model(action: &OdooAction) -> &str {
    match action.target() {
        Some(ActionTarget::Existing { model, .. }) => model,
        _ => "crm.lead",
    }
}

async fn audit_verdict(
    store: &KnowledgeStore,
    doc: &ProposalDocument,
    request: &ApprovalRequest,
    approved: bool,
) {
    let Ok(pg) = store.write_pool() else {
        return;
    };
    let stamp = approver_stamp(request, if approved { "approved" } else { "denied" });
    let approver = stamp.user_id.clone();
    let session_id = SessionId::generate();
    let trace_id = doc.id.to_string();
    let roles = ["admin".to_owned()];
    let call_id = systemprompt::identifiers::CallId::new(request.call_id.clone());
    record_verdict(
        pg.as_ref(),
        &DecisionSubject {
            call_id: &call_id,
            server_name: SERVER_NAME,
            tool_name: TOOL_APPLY_PROPOSAL,
        },
        &DecisionPrincipal {
            user_id: &approver,
            session_id: &session_id,
            roles: &roles,
            client_id: None,
            trace_id: Some(&trace_id),
            act_chain: &[],
        },
        stamp,
        approved,
    )
    .await;
}
