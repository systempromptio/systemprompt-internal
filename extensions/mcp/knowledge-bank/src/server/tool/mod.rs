//! Per-call logic for the `knowledge-bank` server: authentication, the admin
//! gate on uploads, and the three tool handlers.
//!
//! Read tools (`search_project_context`, `list_documents`) are open to any
//! role the registry grants the server to; `upload_document` additionally
//! requires the admin role on the authenticated user — the same double-gate
//! pattern the admin surface uses. The registry grant alone is not enough,
//! because a role edit that widened read access would otherwise silently
//! widen write access with it.

pub mod handlers;
pub mod render;

pub use render::{NO_DOCUMENTS, NO_MATCHES, listing_summary, search_summary};

use crate::store::KnowledgeStore;
use crate::tools::{TOOL_LIST, TOOL_SEARCH, TOOL_UPLOAD};
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RequestContext, RoleServer};
use systemprompt::database::DbPool;
use systemprompt::mcp::middleware::enforce_rbac_from_registry;
use systemprompt::mcp::{ClientProfile, McpToolExecutor};
use systemprompt::models::auth::AuthenticatedUser;
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::{record_mcp_access, record_mcp_access_rejected};

use handlers::{ListHandler, SearchHandler, UploadHandler};

#[doc(hidden)]
#[derive(Debug)]
pub struct Dispatch<'a> {
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub request_context: &'a SysRequestContext,
    pub client: &'a ClientProfile,
}

impl Dispatch<'_> {
    async fn run<H: systemprompt::mcp::McpToolHandler>(
        &self,
        handler: &H,
    ) -> Result<CallToolResult, McpError> {
        self.executor
            .execute(handler, self.request, self.request_context, self.client)
            .await
    }
}

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
            match result.expect_authenticated(
                "BUG: knowledge-bank requires OAuth but auth was not enforced",
            ) {
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

/// The admin gate on `upload_document`.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the gate without a live MCP transport; not part of the public API.
#[doc(hidden)]
pub fn require_admin(request_context: &SysRequestContext) -> Result<(), McpError> {
    let is_admin = request_context
        .user
        .as_ref()
        .is_some_and(AuthenticatedUser::is_admin);
    if is_admin {
        Ok(())
    } else {
        Err(McpError::invalid_request(
            "upload_document requires the admin role; your account can search and list but not \
             upload"
                .to_owned(),
            None,
        ))
    }
}

/// Route one authenticated tool call to its handler.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can drive
/// every branch — the three handlers and the unknown-tool arm — without an
/// rmcp `Peer`, which only exists once a transport is serving. `call_tool`
/// itself is therefore unreachable from a test process; this is the seam that
/// makes its body testable. Not part of the public API.
#[doc(hidden)]
pub async fn dispatch_tool(
    ctx: &Dispatch<'_>,
    store: &KnowledgeStore,
    tool_name: &str,
) -> Result<CallToolResult, McpError> {
    match tool_name {
        TOOL_SEARCH => {
            let handler = SearchHandler {
                store: store.clone(),
            };
            ctx.run(&handler).await
        },
        TOOL_LIST => {
            let handler = ListHandler {
                store: store.clone(),
            };
            ctx.run(&handler).await
        },
        TOOL_UPLOAD => {
            require_admin(ctx.request_context)?;
            let handler = UploadHandler {
                store: store.clone(),
            };
            ctx.run(&handler).await
        },
        _ => Err(McpError::invalid_params(
            format!(
                "Unknown tool: '{tool_name}'. Available tools: {TOOL_SEARCH}, {TOOL_LIST}, \
                 {TOOL_UPLOAD}."
            ),
            None,
        )),
    }
}
