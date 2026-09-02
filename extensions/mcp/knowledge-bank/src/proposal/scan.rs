//! Running the instance's own `secret_scan` policy over a body this pipeline
//! is about to push into Odoo.
//!
//! An inbound email is untrusted text, and posting it to a CRM record is the
//! same exfiltration surface a tool argument would be — so it gets the same
//! scanner, the one configured in `services/governance/config.yaml`, rather
//! than a second pattern list that would drift from it.

use systemprompt::identifiers::{CallId, McpToolName, SessionId, UserId};
use systemprompt::security::authz::Decision;
use systemprompt::security::policy::types::AccessScope;
use systemprompt::security::policy::{
    AgentScope, GovernanceEngine, GovernedInput, GovernedTarget, McpToolInput, PolicyContext,
};

use super::TOOL_APPLY_PROPOSAL;

/// What the scanner said about a body.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScanVerdict {
    Clean,
    Withheld(String),
}

#[must_use]
pub fn scan_body(user_id: &UserId, body: &str) -> ScanVerdict {
    let Some((_, policy)) = GovernanceEngine::global()
        .policies()
        .find(|(config, _)| config.id == "secret_scan" && config.enabled)
    else {
        return ScanVerdict::Clean;
    };

    // JSON: protocol boundary — the scanner reads tool arguments, so the body
    // is presented as one.
    let input =
        GovernedInput::tool_arguments(McpToolInput::new(serde_json::json!({ "body": body })));
    let session_id = SessionId::generate();
    let call_id = CallId::new(format!("scan-{}", uuid::Uuid::new_v4()));
    let ctx = PolicyContext {
        target: GovernedTarget::Tool {
            tool: McpToolName::new(TOOL_APPLY_PROPOSAL),
        },
        agent_scope: AgentScope::System,
        access_scope: AccessScope::from_roles::<String>(&[]),
        session_id: &session_id,
        user_id,
        input: &input,
        call_id: &call_id,
    };
    match policy.evaluate(&ctx) {
        Decision::Deny { reason } => ScanVerdict::Withheld(reason.to_string()),
        Decision::Allow { .. } | Decision::Pending { .. } => ScanVerdict::Clean,
    }
}
