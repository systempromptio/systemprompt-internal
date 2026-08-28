//! The `governance_decisions` writer for approval milestones.
//!
//! A held call leaves three rows rather than one: `pending` when it is
//! flagged, then `allow` carrying the approver, or `deny` carrying whoever
//! refused it. Splitting them out keeps the gate itself readable as a single
//! decision path.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.

use systemprompt::identifiers::{CallId, PolicyId};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::Decision;
use systemprompt::security::policy::types::AccessScope;
use systemprompt::security::policy::{
    ApproverStamp, AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult, DecisionAudit,
    PrincipalSnapshot, record_decision,
};

// Why: grouped rather than passed loose for the same reason `ApprovalVerdict`
// is — a nine-argument call at three sites is where a caller eventually
// transposes two `&str`s.
pub(super) struct Milestone<'a> {
    pub call_id: &'a CallId,
    pub server_name: &'a str,
    pub tool_name: &'a str,
    pub ctx: &'a SysRequestContext,
    pub decision: Decision,
    pub result: ChainEntryResult,
    pub detail: String,
    pub approver: Option<ApproverStamp>,
}

// Why: a lost audit write must never flip the gate's answer, so every failure
// is logged here and nothing propagates to the caller.
pub(super) async fn audit(pool: &sqlx::PgPool, milestone: Milestone<'_>) {
    let Milestone {
        call_id,
        server_name,
        tool_name,
        ctx,
        decision,
        result,
        detail,
        approver,
    } = milestone;

    let roles = ctx
        .user
        .as_ref()
        .map(|u| u.roles.clone())
        .unwrap_or_default();
    let audit = DecisionAudit {
        id: uuid::Uuid::new_v4().to_string(),
        call_id: call_id.as_str().to_owned(),
        origin: AuditOrigin::Governed,
        decision,
        principal: PrincipalSnapshot {
            user_id: ctx.user_id().clone(),
            session_id: ctx.session_id().clone(),
            agent_session: None,
            agent_id: None,
            agent_scope: AccessScope::from_roles(&roles),
            client_id: ctx.client_id().cloned(),
            claimed: None,
        },
        target: AuditTarget {
            tool_name: tool_name.to_owned(),
            plugin_id: None,
        },
        chain: vec![ChainEntryOutcome {
            policy_id: PolicyId::new("require_approval"),
            result,
            detail,
            duration_ms: 0.0,
        }],
        approver,
        act_chain: ctx.act_chain().to_vec(),
        context_id: None,
        trace_id: Some(ctx.trace_id().as_str().to_owned()),
    };
    if let Err(err) = record_decision(pool, &audit).await {
        tracing::error!(
            tool_name,
            server_name,
            error = %err,
            "could not record the approval decision"
        );
    }
}
