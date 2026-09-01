//! The `governance_decisions` writer for approval milestones, and the stamp
//! that names who answered.
//!
//! A held call leaves three rows rather than one: `pending` when it is
//! flagged, then `allow` carrying the approver, or `deny` carrying whoever
//! refused it. The writer takes the principal as a value rather than a
//! request context so that a hold opened by a scheduled job — which has a user
//! but no live MCP request — records the same three rows as a hold opened
//! mid-call. Both paths share this one writer so the audit trail cannot
//! diverge by origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.

use systemprompt::identifiers::{
    Actor, CallId, ClientId, McpToolName, PolicyId, SessionId, UserId,
};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::{Decision, DenyReason, MatchedBy, PendingReason};
use systemprompt::security::policy::types::AccessScope;
use systemprompt::security::policy::{
    ApprovalRequest, ApproverStamp, AuditOrigin, AuditTarget, ChainEntryOutcome, ChainEntryResult,
    DecisionAudit, PrincipalSnapshot, record_decision,
};

const POLICY_ID: &str = "require_approval";

/// Who the decision is recorded for.
#[derive(Debug, Clone)]
pub struct DecisionPrincipal<'a> {
    pub user_id: &'a UserId,
    pub session_id: &'a SessionId,
    pub roles: &'a [String],
    pub client_id: Option<&'a ClientId>,
    pub trace_id: Option<&'a str>,
    pub act_chain: &'a [Actor],
}

impl<'a> DecisionPrincipal<'a> {
    #[must_use]
    pub fn from_request_context(ctx: &'a SysRequestContext) -> Self {
        Self {
            user_id: ctx.user_id(),
            session_id: ctx.session_id(),
            roles: ctx.user.as_ref().map_or(&[], |u| u.roles.as_slice()),
            client_id: ctx.client_id(),
            trace_id: Some(ctx.trace_id().as_str()),
            act_chain: ctx.act_chain(),
        }
    }
}

/// Which call the decision is about.
#[derive(Debug, Clone, Copy)]
pub struct DecisionSubject<'a> {
    pub call_id: &'a CallId,
    pub server_name: &'a str,
    pub tool_name: &'a str,
}

pub async fn record_hold(
    pool: &sqlx::PgPool,
    subject: &DecisionSubject<'_>,
    principal: &DecisionPrincipal<'_>,
    rule: &str,
) {
    let decision = Decision::Pending {
        reason: PendingReason::ApprovalRequired {
            tool: McpToolName::new(subject.tool_name),
            rule: rule.to_owned(),
        },
    };
    let judgement = Judgement {
        decision,
        result: ChainEntryResult::Hold,
        detail: format!("Held for human approval by rule {rule}"),
        approver: None,
    };
    write(pool, subject, principal, judgement).await;
}

pub async fn record_verdict(
    pool: &sqlx::PgPool,
    subject: &DecisionSubject<'_>,
    principal: &DecisionPrincipal<'_>,
    stamp: ApproverStamp,
    approved: bool,
) {
    let who = stamp.username.clone();
    let (decision, result, detail) = if approved {
        (
            Decision::Allow {
                matched_by: MatchedBy::PolicyAllow {
                    policy_id: PolicyId::new(POLICY_ID),
                    detail: std::borrow::Cow::Borrowed("Approved by a human"),
                },
            },
            ChainEntryResult::Pass,
            format!("Approved by {who}"),
        )
    } else {
        (
            Decision::Deny {
                reason: DenyReason::PolicyViolation {
                    policy: POLICY_ID.to_owned(),
                    detail: std::borrow::Cow::Owned(format!("Refused by {who}")),
                },
            },
            ChainEntryResult::Fail,
            format!("Refused by {who}"),
        )
    };
    let judgement = Judgement {
        decision,
        result,
        detail,
        approver: Some(stamp),
    };
    write(pool, subject, principal, judgement).await;
}

// Why: a resolved row always has an approver, but the columns are nullable
// because a pending row must not carry one. The fallbacks are unreachable in
// practice and exist so a decision is never dropped for want of a name.
#[must_use]
pub fn approver_stamp(request: &ApprovalRequest, action: &'static str) -> ApproverStamp {
    ApproverStamp {
        user_id: UserId::new(request.approver_id.clone().unwrap_or_default()),
        username: request
            .approver_username
            .clone()
            .unwrap_or_else(|| "an approver".to_owned()),
        decided_at: request.decided_at.unwrap_or_else(chrono::Utc::now),
        action,
    }
}

struct Judgement {
    decision: Decision,
    result: ChainEntryResult,
    detail: String,
    approver: Option<ApproverStamp>,
}

// Why: a lost audit write must never flip the gate's answer, so every failure
// is logged here and nothing propagates to the caller.
async fn write(
    pool: &sqlx::PgPool,
    subject: &DecisionSubject<'_>,
    principal: &DecisionPrincipal<'_>,
    judgement: Judgement,
) {
    let Judgement {
        decision,
        result,
        detail,
        approver,
    } = judgement;
    let audit = DecisionAudit {
        id: uuid::Uuid::new_v4().to_string(),
        call_id: subject.call_id.as_str().to_owned(),
        origin: AuditOrigin::Governed,
        decision,
        principal: PrincipalSnapshot {
            user_id: principal.user_id.clone(),
            session_id: principal.session_id.clone(),
            agent_session: None,
            agent_id: None,
            agent_scope: AccessScope::from_roles(principal.roles),
            client_id: principal.client_id.cloned(),
            claimed: None,
        },
        target: AuditTarget {
            tool_name: subject.tool_name.to_owned(),
            plugin_id: None,
        },
        chain: vec![ChainEntryOutcome {
            policy_id: PolicyId::new(POLICY_ID),
            result,
            detail,
            duration_ms: 0.0,
        }],
        approver,
        act_chain: principal.act_chain.to_vec(),
        context_id: None,
        trace_id: principal.trace_id.map(str::to_owned),
    };
    if let Err(err) = record_decision(pool, &audit).await {
        tracing::error!(
            tool_name = subject.tool_name,
            server_name = subject.server_name,
            error = %err,
            "could not record the approval decision"
        );
    }
}
