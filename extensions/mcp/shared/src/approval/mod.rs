//! The human-approval gate, shared by every bundled MCP server.
//!
//! Call it from `ServerHandler::call_tool` **after** the server has
//! authenticated the caller and **before** it resolves any backend credential
//! or does any work. Held that way a parked call costs nothing: no credential
//! is resolved, no upstream round trip is made, and no model spend is incurred
//! until a human says yes.
//!
//! # Why the call id is derived, not generated
//!
//! SEP-2322 rounds are stateless: the client retries the *same* `tools/call`,
//! and nothing in the retry carries a server-assigned identity that we may
//! trust. `CallId` is therefore derived from the caller, the server, the tool
//! and a digest of the arguments, which makes it stable across rounds by
//! construction. That is also what makes
//! `GovernancePolicy::evaluate`'s per-call idempotency contract hold here — a
//! retry rejoins the approval it opened rather than opening a second one.
//!
//! # Why `requestState` needs no sealing
//!
//! The spec is explicit that the client echoes `requestState` back verbatim
//! and that a server storing meaning in it must verify integrity — rmcp ships
//! `RequestStateCodec` for exactly that. We sidestep the problem instead of
//! solving it: because the authoritative call id is *recomputed* from the
//! retried request on every round, a tampered or replayed `requestState`
//! changes nothing. It is echoed for debuggability and spec shape only, and is
//! never read back as trusted input.

use rmcp::model::{CallToolRequestParams, CallToolResult, ContentBlock, InputRequiredResult};
use sha2::{Digest, Sha256};
use systemprompt::database::DbPool;
use systemprompt::identifiers::{CallId, McpToolName};

mod audit;
mod settle;

use audit::Milestone;
use settle::settle;
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::{Decision, PendingReason};
use systemprompt::security::policy::types::AccessScope;
use systemprompt::security::policy::{
    AgentScope, ApprovalRepository, ApprovalSettings, ApproverStamp, ChainEntryResult,
    GovernanceEngine, GovernedInput, GovernedTarget, McpToolInput, NewApprovalRequest,
    PolicyContext,
};

/// What the gate decided the caller should do next.
#[derive(Debug)]
pub enum GateOutcome {
    // Why: `Proceed` carries no approver stamp — the gate writes it to the
    // audit itself, so a consuming server never has to handle it.
    Proceed,
    Held(Box<InputRequiredResult>),
    Refused(Box<CallToolResult>),
}

// Why: carries the arguments and the derived call id forward so neither is
// recomputed once the verdict is known — recomputing either is how the two
// halves of a hold drift apart.
pub(super) struct Held<'a> {
    pub(super) call_id: CallId,
    pub(super) rule: String,
    pub(super) arguments: serde_json::Value,
    pub(super) settings: ApprovalSettings,
    pub(super) server_name: &'a str,
    pub(super) tool_name: &'a str,
    pub(super) ctx: &'a SysRequestContext,
}

// Why: only `require_approval` is evaluated here, not the whole chain. The
// rest runs at the enforcement points that already own it, and reaching for
// the whole engine would charge the shared rate limiter twice for one call.
pub async fn enforce_approval(
    db_pool: &DbPool,
    server_name: &str,
    tool_name: &str,
    request: &CallToolRequestParams,
    request_context: &SysRequestContext,
) -> GateOutcome {
    let Some(held) = held_call(server_name, tool_name, request, request_context) else {
        return GateOutcome::Proceed;
    };

    let Some(pg_pool) = db_pool.pool() else {
        // Why: the approval row IS the gate. With no database there is nowhere
        // to record the hold and no way for a human to answer it, so the only
        // honest outcome is a refusal — letting the call through would run an
        // unapproved write and report it as approved.
        tracing::error!(
            tool_name,
            "no database available to hold a call for approval; refusing it"
        );
        return GateOutcome::Refused(Box::new(refusal(
            "This tool requires human approval, but the approval store is unavailable.",
        )));
    };
    let repo = ApprovalRepository::new((*pg_pool).clone());

    if let Err(err) = open_hold(&repo, &held).await {
        tracing::error!(tool_name, error = %err, "could not open an approval request");
        return GateOutcome::Refused(Box::new(refusal(
            "This tool requires human approval, but the request could not be recorded.",
        )));
    }

    settle(&repo, &pg_pool, &held).await
}

impl<'a> Held<'a> {
    const fn milestone(
        &'a self,
        decision: Decision,
        result: ChainEntryResult,
        detail: String,
        approver: Option<ApproverStamp>,
    ) -> Milestone<'a> {
        Milestone {
            call_id: &self.call_id,
            server_name: self.server_name,
            tool_name: self.tool_name,
            ctx: self.ctx,
            decision,
            result,
            detail,
            approver,
        }
    }
}

fn held_call<'a>(
    server_name: &'a str,
    tool_name: &'a str,
    request: &CallToolRequestParams,
    ctx: &'a SysRequestContext,
) -> Option<Held<'a>> {
    let arguments = request
        .arguments
        .clone()
        .map_or(serde_json::Value::Null, serde_json::Value::Object);

    let (policy_config, policy) = GovernanceEngine::global()
        .policies()
        .find(|(config, _)| config.id == "require_approval" && config.enabled)?;

    let call_id = derive_call_id(ctx, server_name, tool_name, &arguments);
    let roles = ctx
        .user
        .as_ref()
        .map(|u| u.roles.clone())
        .unwrap_or_default();
    let input = GovernedInput::tool_arguments(McpToolInput::new(arguments.clone()));

    let policy_ctx = PolicyContext {
        target: GovernedTarget::Tool {
            tool: McpToolName::new(tool_name),
        },
        agent_scope: AgentScope::User {
            user_id: ctx.user_id().clone(),
        },
        access_scope: AccessScope::from_roles(&roles),
        session_id: ctx.session_id(),
        user_id: ctx.user_id(),
        input: &input,
        call_id: &call_id,
    };

    let Decision::Pending {
        reason: PendingReason::ApprovalRequired { rule, .. },
    } = policy.evaluate(&policy_ctx)
    else {
        return None;
    };

    Some(Held {
        call_id,
        rule,
        arguments,
        settings: ApprovalSettings::from_params(&policy_config.params),
        server_name,
        tool_name,
        ctx,
    })
}

// Why: `open` is insert-if-absent, not upsert. SEP-2322 retries re-enter this
// path with the same derived call id, so an upsert would reset the deadline
// every round and the call could never expire.
async fn open_hold(repo: &ApprovalRepository, held: &Held<'_>) -> Result<(), sqlx::Error> {
    repo.open(&NewApprovalRequest {
        call_id: &held.call_id,
        tool_name: held.tool_name,
        server_name: held.server_name,
        arguments: &held.arguments,
        requested_by: held.ctx.user_id(),
        session_id: Some(held.ctx.session_id()),
        trace_id: Some(held.ctx.trace_id().as_str()),
        rule: &held.rule,
        expires_in_seconds: held.settings.expiry_seconds,
    })
    .await?;
    Ok(())
}

// Why: a stable identity for one logical call across MRTR rounds, derived
// from `request.arguments` and NEVER from `input_responses` or
// `request_state`. That is load-bearing: a server may run its own elicitation
// rounds before reaching this gate, and those change `input_responses` every
// retry. Feeding them into the digest would move the id between rounds, so
// each retry would open a fresh approval and the hold could never converge.
fn derive_call_id(
    ctx: &SysRequestContext,
    server_name: &str,
    tool_name: &str,
    arguments: &serde_json::Value,
) -> CallId {
    let mut hasher = Sha256::new();
    hasher.update(ctx.user_id().as_str().as_bytes());
    hasher.update(b"\x1f");
    hasher.update(server_name.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(tool_name.as_bytes());
    hasher.update(b"\x1f");
    hasher.update(systemprompt::security::policy::args_digest(arguments).as_bytes());
    CallId::new(format!("{:x}", hasher.finalize()))
}

// Why: a governance refusal comes back as an isError result rather than a
// JSON-RPC error, matching how this server already reports link and setup
// states — strict bridges reject a JSON-RPC error before anything renders it.
fn refusal(message: &str) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message.to_owned())])
}
