//! Per-call logic: authenticate the platform caller, resolve their Odoo
//! credential, then route to a handler.
//!
//! The two authentications are distinct and both required. The MCP transport
//! proves *which platform user* is calling; `odoo_identity` turns that into an
//! Odoo credential, and Odoo then decides what that credential may do. This
//! server enforces no data-access policy of its own — it has none to enforce
//! that Odoo does not already own.

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RequestContext, RoleServer};
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::mcp::middleware::enforce_rbac_from_registry;
use systemprompt::mcp::{McpToolExecutor, McpToolHandler};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::{record_mcp_access, record_mcp_access_rejected};

use super::call::OdooCall;
use super::{crm, notes, overview, partner, report};
use crate::client::OdooClient;
use crate::identity::resolve_credentials;
use crate::tools::{
    ALL_TOOLS, TOOL_ACTIVITY_LIST, TOOL_LEAD_CREATE, TOOL_LEAD_GET, TOOL_LEAD_REPORT,
    TOOL_LEAD_SEARCH, TOOL_LEAD_UPDATE, TOOL_NOTE_ADD, TOOL_OVERVIEW, TOOL_PARTNER_GET,
    TOOL_PARTNER_SEARCH,
};

pub(super) async fn authenticate_tool_request(
    db_pool: &DbPool,
    tool_name: &str,
    service_id: &str,
    ctx: &RequestContext<RoleServer>,
    authz_hook: &SharedAuthzHook,
) -> Result<SysRequestContext, McpError> {
    let rbac_result = enforce_rbac_from_registry(ctx, service_id, authz_hook).await;

    match rbac_result {
        Ok(result) => {
            match result.expect_authenticated("BUG: odoo requires OAuth but auth was not enforced")
            {
                Ok(authenticated) => {
                    record_mcp_access(
                        db_pool,
                        authenticated.context.user_id(),
                        service_id,
                        tool_name,
                        "authenticated",
                    )
                    .await;
                    Ok(authenticated.context.clone())
                },
                Err(e) => {
                    record_mcp_access_rejected(db_pool, service_id, tool_name, e.message.as_ref())
                        .await;
                    Err(e)
                },
            }
        },
        Err(e) => {
            record_mcp_access_rejected(db_pool, service_id, tool_name, &format!("{e}")).await;
            Err(e)
        },
    }
}

// Why: builds the per-request call bundle. A caller with no linked Odoo
// account fails here, before any handler runs, with a message naming the
// profile page — never with an empty result.
pub(super) async fn build_call(
    db_pool: &DbPool,
    client: &Arc<OdooClient>,
    request_context: &SysRequestContext,
) -> Result<OdooCall, McpError> {
    let creds = resolve_credentials(db_pool, request_context.user_id()).await?;
    Ok(OdooCall {
        client: Arc::clone(client),
        creds,
    })
}

/// Route one authenticated tool call to its handler.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can drive
/// every arm — including the unknown-tool arm — without an rmcp `Peer`, which
/// only exists once a transport is serving. `call_tool` itself is therefore
/// unreachable from a test process; this is the seam that makes its body
/// testable. Not part of the public API.
#[doc(hidden)]
pub async fn dispatch_tool(
    executor: &McpToolExecutor,
    call: OdooCall,
    tool_name: &str,
    request: &CallToolRequestParams,
    request_context: &SysRequestContext,
) -> Result<CallToolResult, McpError> {
    match tool_name {
        TOOL_LEAD_SEARCH => run(executor, &crm::LeadSearchHandler { call }, request, request_context).await,
        TOOL_LEAD_GET => run(executor, &crm::LeadGetHandler { call }, request, request_context).await,
        TOOL_LEAD_CREATE => run(executor, &crm::LeadCreateHandler { call }, request, request_context).await,
        TOOL_LEAD_UPDATE => run(executor, &crm::LeadUpdateHandler { call }, request, request_context).await,
        TOOL_LEAD_REPORT => run(executor, &report::LeadReportHandler { call }, request, request_context).await,
        TOOL_PARTNER_SEARCH => {
            run(executor, &partner::PartnerSearchHandler { call }, request, request_context).await
        },
        TOOL_PARTNER_GET => {
            run(executor, &partner::PartnerGetHandler { call }, request, request_context).await
        },
        TOOL_NOTE_ADD => run(executor, &notes::NoteAddHandler { call }, request, request_context).await,
        TOOL_ACTIVITY_LIST => {
            run(executor, &notes::ActivityListHandler { call }, request, request_context).await
        },
        TOOL_OVERVIEW => {
            run(executor, &overview::OverviewHandler { call }, request, request_context).await
        },
        _ => Err(unknown_tool(tool_name)),
    }
}

async fn run<H: McpToolHandler>(
    executor: &McpToolExecutor,
    handler: &H,
    request: &CallToolRequestParams,
    request_context: &SysRequestContext,
) -> Result<CallToolResult, McpError> {
    executor.execute(handler, request, request_context).await
}

/// The unknown-tool error, listing what this server does answer to.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the message names every tool; not part of the public API.
#[doc(hidden)]
#[must_use]
pub fn unknown_tool(tool_name: &str) -> McpError {
    McpError::invalid_params(
        format!(
            "Unknown tool: '{tool_name}'. Available tools: {}.",
            ALL_TOOLS.join(", ")
        ),
        None,
    )
}
