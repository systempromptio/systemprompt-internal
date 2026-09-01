//! Waiting out one round of a hold and turning the answer into an outcome.
//!
//! Split from the gate itself so `mod.rs` reads as the decision path — is this
//! call held, can it be recorded, what happened — with the three-way landing
//! (approved / refused / still waiting) and its audit rows kept here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.

use std::time::Duration;

use rmcp::model::InputRequiredResult;
use systemprompt::identifiers::{McpToolName, PolicyId, UserId};
use systemprompt::security::authz::{Decision, DenyReason, MatchedBy, PendingReason};
use systemprompt::security::policy::{
    ApprovalOutcome, ApprovalRepository, ApprovalRequest, ApproverStamp, ChainEntryResult,
    wait_for_decision,
};

use super::audit::audit;
use super::{GateOutcome, Held, refusal};

pub(super) async fn settle(
    repo: &ApprovalRepository,
    pool: &sqlx::PgPool,
    held: &Held<'_>,
) -> GateOutcome {
    let tool_name = held.tool_name;

    tracing::info!(
        tool_name,
        call_id = %held.call_id,
        rule = %held.rule,
        "tool call held for human approval"
    );

    // Why: written once per call, not once per round — a retried round
    // re-opens nothing, so this cannot duplicate.
    audit(
        pool,
        held.milestone(
            Decision::Pending {
                reason: PendingReason::ApprovalRequired {
                    tool: McpToolName::new(tool_name),
                    rule: held.rule.clone(),
                },
            },
            ChainEntryResult::Hold,
            format!("Held for human approval by rule {}", held.rule),
            None,
        ),
    )
    .await;

    let waited = wait_for_decision(
        repo,
        held.call_id.as_str(),
        Duration::from_secs(held.settings.hold_seconds),
    )
    .await;

    match waited {
        ApprovalOutcome::Approved(request) => on_approved(pool, held, &request).await,
        ApprovalOutcome::Denied(request) => on_denied(pool, held, &request).await,
        ApprovalOutcome::Expired(_) => GateOutcome::Refused(Box::new(refusal(&format!(
            "{tool_name} required human approval and none was given in time."
        )))),
        ApprovalOutcome::StillPending(_) => GateOutcome::Held(Box::new(
            InputRequiredResult::from_request_state(held.call_id.as_str().to_owned()),
        )),
    }
}

async fn on_approved(
    pool: &sqlx::PgPool,
    held: &Held<'_>,
    request: &ApprovalRequest,
) -> GateOutcome {
    let stamp = stamp(request, "approved");
    tracing::info!(
        tool_name = held.tool_name,
        call_id = %held.call_id,
        approver = %stamp.username,
        "held tool call approved"
    );
    let detail = format!("Approved by {}", stamp.username);
    audit(
        pool,
        held.milestone(
            Decision::Allow {
                matched_by: MatchedBy::PolicyAllow {
                    policy_id: PolicyId::new("require_approval"),
                    detail: std::borrow::Cow::Borrowed("Approved by a human"),
                },
            },
            ChainEntryResult::Pass,
            detail,
            Some(stamp),
        ),
    )
    .await;
    GateOutcome::Proceed
}

async fn on_denied(pool: &sqlx::PgPool, held: &Held<'_>, request: &ApprovalRequest) -> GateOutcome {
    let stamp = stamp(request, "denied");
    let who = stamp.username.clone();
    audit(
        pool,
        held.milestone(
            Decision::Deny {
                reason: DenyReason::PolicyViolation {
                    policy: "require_approval".to_owned(),
                    detail: std::borrow::Cow::Owned(format!("Refused by {who}")),
                },
            },
            ChainEntryResult::Fail,
            format!("Refused by {who}"),
            Some(stamp),
        ),
    )
    .await;
    GateOutcome::Refused(Box::new(refusal(&format!(
        "{} was refused by {who}.{}",
        held.tool_name,
        request
            .decision_note
            .as_deref()
            .map_or_else(String::new, |n| format!(" Reason: {n}"))
    ))))
}

// Why: a resolved row always has an approver, but the columns are nullable
// because a pending row must not carry one. The fallbacks are unreachable in
// practice and exist so a decision is never dropped for want of a name.
fn stamp(request: &ApprovalRequest, action: &'static str) -> ApproverStamp {
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
