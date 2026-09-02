//! The admin gate on `upload_document`.
//!
//! The registry grant that lets a role reach the knowledge bank at all is not
//! enough to write to it: `require_admin` re-checks the authenticated user, so
//! a roles.yaml edit that widened read access cannot silently widen write
//! access with it. This is the second half of the double-gate the server's
//! yaml describes, and it is asserted here rather than through dispatch
//! because it is a pure function of the request context.

use systemprompt::identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt::models::auth::{AuthenticatedUser, Permission};
use systemprompt::models::execution::context::RequestContext;
use systemprompt_mcp_knowledge_bank::server::tool::require_admin;
use systemprompt_mcp_knowledge_bank::tools::{TOOL_PROPOSAL_DECIDE, TOOL_UPLOAD};

fn anonymous() -> RequestContext {
    RequestContext::new(
        SessionId::new("kb-gate-session"),
        TraceId::new("kb-gate-trace"),
        ContextId::new_unchecked("00000000-0000-4000-8000-00000000e46e"),
        AgentName::new("kb-gate-agent"),
    )
}

fn signed_in_as(permission: Permission) -> RequestContext {
    anonymous().with_user(AuthenticatedUser::new(
        uuid::Uuid::new_v4(),
        "kb-caller".to_owned(),
        "kb-caller@example.com".to_owned(),
        vec![permission],
    ))
}

#[test]
fn an_admin_may_upload() {
    assert!(require_admin(&signed_in_as(Permission::Admin), TOOL_UPLOAD).is_ok());
    assert!(require_admin(&signed_in_as(Permission::Admin), TOOL_PROPOSAL_DECIDE).is_ok());
}

#[test]
fn a_signed_in_non_admin_is_refused() {
    let error = require_admin(&signed_in_as(Permission::User), TOOL_UPLOAD)
        .expect_err("the user role can read but not write");
    assert!(
        error.message.contains("requires the admin role"),
        "the refusal names the missing role: {}",
        error.message
    );
    assert!(
        error.message.contains("search and list"),
        "the refusal says what the caller can still do: {}",
        error.message
    );
}

#[test]
fn an_anonymous_context_is_refused() {
    // Belt and braces: transport auth should never let this context reach the
    // handler, but the gate must not depend on that being true.
    assert!(require_admin(&anonymous(), TOOL_UPLOAD).is_err());
}

#[test]
fn the_proposal_tools_are_gated_the_same_way() {
    let error = require_admin(&signed_in_as(Permission::User), TOOL_PROPOSAL_DECIDE)
        .expect_err("proposals carry inbound business email");
    assert!(error.message.contains(TOOL_PROPOSAL_DECIDE));
    assert!(error.message.contains("ingestion proposals"));
}
