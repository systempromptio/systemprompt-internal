//! Waiting out one round of a hold and turning the answer into an outcome.
//!
//! Split from the gate itself so `mod.rs` reads as the decision path — is this
//! call held, can it be recorded, what happened — with the three-way landing
//! (approved / refused / still waiting) and its audit rows kept here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.

use std::time::Duration;

use rmcp::model::InputRequiredResult;
use systemprompt::security::policy::{ApprovalOutcome, ApprovalRepository, wait_for_decision};

use super::resolution::{
    DecisionPrincipal, DecisionSubject, approver_stamp, record_hold, record_verdict,
};
use super::{GateOutcome, Held, refusal};

pub(super) async fn settle(
    repo: &ApprovalRepository,
    pool: &sqlx::PgPool,
    held: &Held<'_>,
) -> GateOutcome {
    let tool_name = held.tool_name;
    let subject = DecisionSubject {
        call_id: &held.call_id,
        server_name: held.server_name,
        tool_name,
    };
    let principal = DecisionPrincipal::from_request_context(held.ctx);

    tracing::info!(
        tool_name,
        call_id = %held.call_id,
        rule = %held.rule,
        "tool call held for human approval"
    );

    // Why: written once per call, not once per round — a retried round
    // re-opens nothing, so this cannot duplicate.
    record_hold(pool, &subject, &principal, &held.rule).await;

    let waited = wait_for_decision(
        repo,
        held.call_id.as_str(),
        Duration::from_secs(held.settings.hold_seconds),
    )
    .await;

    match waited {
        ApprovalOutcome::Approved(request) => {
            let stamp = approver_stamp(&request, "approved");
            tracing::info!(
                tool_name,
                call_id = %held.call_id,
                approver = %stamp.username,
                "held tool call approved"
            );
            record_verdict(pool, &subject, &principal, stamp, true).await;
            GateOutcome::Proceed
        },
        ApprovalOutcome::Denied(request) => {
            let stamp = approver_stamp(&request, "denied");
            let who = stamp.username.clone();
            record_verdict(pool, &subject, &principal, stamp, false).await;
            GateOutcome::Refused(Box::new(refusal(&format!(
                "{tool_name} was refused by {who}.{}",
                request
                    .decision_note
                    .as_deref()
                    .map_or_else(String::new, |n| format!(" Reason: {n}"))
            ))))
        },
        ApprovalOutcome::Expired(_) => GateOutcome::Refused(Box::new(refusal(&format!(
            "{tool_name} required human approval and none was given in time."
        )))),
        ApprovalOutcome::StillPending(_) => GateOutcome::Held(Box::new(
            InputRequiredResult::from_request_state(held.call_id.as_str().to_owned()),
        )),
    }
}
