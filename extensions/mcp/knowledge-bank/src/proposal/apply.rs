//! Applying an approved proposal, one action at a time.
//!
//! Each action is one ledger claim, one Odoo write and one ledger finish, in
//! order, with later actions able to land on the lead an earlier one created.
//! The writes themselves live in [`super::writes`].

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_mcp_odoo::client::{Credentials, OdooClient};
use systemprompt_mcp_odoo::error::OdooError;
use uuid::Uuid;

use super::ledger::{Claim, LedgerKey, NewProjection, claim_action, fail_action, finish_action};
use super::tags::tag_record;
use super::writes::{create_activity, create_lead, create_task, post_chatter};
use super::{ActionTarget, OdooAction};

/// What every write in one proposal shares.
#[derive(Debug, Clone, Copy)]
pub struct ApplyContext<'a> {
    pub pool: &'a PgPool,
    pub client: &'a OdooClient,
    pub creds: &'a Credentials,
    pub approver: &'a UserId,
}

/// The document being projected, as the writes need it.
#[derive(Debug, Clone, Copy)]
pub struct ApplySource<'a> {
    pub document_id: Uuid,
    pub revision: i32,
    pub rfc5322_id: &'a str,
    pub email_from: &'a str,
    pub subject: &'a str,
    pub body_html: &'a str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AppliedStatus {
    Done,
    Failed,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppliedAction {
    pub index: usize,
    pub kind: String,
    pub label: String,
    pub status: AppliedStatus,
    pub model: Option<String>,
    pub res_id: Option<i64>,
    pub url: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AppliedOutcome {
    pub actions: Vec<AppliedAction>,
    pub all_ok: bool,
}

pub async fn apply_document(
    ctx: &ApplyContext<'_>,
    source: &ApplySource<'_>,
    actions: &[OdooAction],
) -> AppliedOutcome {
    let mut applied: Vec<AppliedAction> = Vec::with_capacity(actions.len());
    for (index, action) in actions.iter().enumerate() {
        let outcome = apply_one(ctx, source, index, action, &applied).await;
        applied.push(outcome);
    }
    let all_ok = applied.iter().all(|a| a.status != AppliedStatus::Failed);
    AppliedOutcome {
        actions: applied,
        all_ok,
    }
}

async fn apply_one(
    ctx: &ApplyContext<'_>,
    source: &ApplySource<'_>,
    index: usize,
    action: &OdooAction,
    earlier: &[AppliedAction],
) -> AppliedAction {
    let key = LedgerKey {
        document_id: source.document_id,
        revision: source.revision,
        action_index: i32::try_from(index).unwrap_or(i32::MAX),
    };
    let mut result = AppliedAction {
        index,
        kind: action.kind().to_owned(),
        label: action.label(),
        status: AppliedStatus::Failed,
        model: None,
        res_id: None,
        url: None,
        error: None,
    };

    let target = match resolve_target(action, earlier) {
        Ok(t) => t,
        Err(e) => {
            result.error = Some(e);
            return result;
        },
    };
    let model: String = target
        .as_ref()
        .map_or_else(|| "crm.lead".to_owned(), |(m, _)| m.clone());
    result.model = Some(model.clone());

    let row = NewProjection {
        key,
        kind: action.kind(),
        res_model: &model,
        rfc5322_id: source.rfc5322_id,
        applied_by: ctx.approver.as_str(),
        odoo_login: &ctx.creds.login,
    };
    if !claim(ctx, &row, target.as_ref().map(|(_, id)| *id), &mut result).await {
        return result;
    }

    let written = match (action, target) {
        (OdooAction::CreateLead { .. }, _) => create_lead(ctx, action).await,
        (OdooAction::PostChatter { .. }, Some((m, id))) => post_chatter(ctx, source, &m, id).await,
        (OdooAction::TagRecord { .. }, Some((m, id))) => tag_record(ctx, action, &m, id).await,
        (OdooAction::CreateActivity { .. }, Some((m, id))) => {
            create_activity(ctx, action, &m, id).await
        },
        (OdooAction::CreateTask { .. }, Some((m, id))) => create_task(ctx, action, &m, id).await,
        (_, None) => Err(OdooError::Internal("action has no target".to_owned())),
    };

    match written {
        Ok((res_id, odoo_message_id)) => {
            if let Err(e) = finish_action(ctx.pool, key, Some(res_id), odoo_message_id).await {
                tracing::error!(error = %e, document_id = %source.document_id, index, "ledger finish failed after a successful Odoo write");
            }
            result.status = AppliedStatus::Done;
            result.res_id = Some(res_id);
            result.url = Some(ctx.client.connection().record_url(&model, res_id));
        },
        Err(e) => {
            let message = e.to_string();
            if let Err(le) = fail_action(ctx.pool, key, &message).await {
                tracing::error!(error = %le, document_id = %source.document_id, index, "ledger fail-mark failed");
            }
            result.error = Some(message);
        },
    }
    result
}

// Why: a claim that finds the row already done or excluded is a finished
// action, and the ledger — not the caller — decides which.
async fn claim(
    ctx: &ApplyContext<'_>,
    row: &NewProjection<'_>,
    target_id: Option<i64>,
    result: &mut AppliedAction,
) -> bool {
    match claim_action(ctx.pool, row).await {
        Ok(Claim::Open) => true,
        Ok(Claim::Excluded) => {
            result.status = AppliedStatus::Excluded;
            false
        },
        Ok(Claim::Done { res_id, .. }) => {
            result.status = AppliedStatus::Done;
            result.res_id = res_id.or(target_id);
            result.url = result
                .res_id
                .map(|id| ctx.client.connection().record_url(row.res_model, id));
            false
        },
        Err(e) => {
            result.error = Some(format!("ledger claim failed: {e}"));
            false
        },
    }
}

// Why: a follow-up aimed at the lead this proposal creates resolves to that
// lead's id only once the create has happened; before it, or if it failed,
// the follow-up cannot run.
fn resolve_target(
    action: &OdooAction,
    earlier: &[AppliedAction],
) -> Result<Option<(String, i64)>, String> {
    match action.target() {
        None => Ok(None),
        Some(ActionTarget::Existing { model, res_id, .. }) => Ok(Some((model.clone(), *res_id))),
        Some(ActionTarget::CreatedLead { action_index }) => earlier
            .get(*action_index)
            .filter(|a| a.status == AppliedStatus::Done)
            .and_then(|a| a.res_id)
            .map(|id| Some(("crm.lead".to_owned(), id)))
            .ok_or_else(|| {
                format!("depends on action {action_index}, which did not create a lead")
            }),
    }
}
