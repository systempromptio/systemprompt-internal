//! Authenticate the caller, then route to a handler.

use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RequestContext, RoleServer};
use systemprompt::database::DbPool;
use systemprompt::mcp::middleware::enforce_rbac_from_registry;
use systemprompt::mcp::{ClientProfile, McpToolExecutor, McpToolHandler};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::{record_mcp_access, record_mcp_access_rejected};

use super::handlers::{Call, GetHandler, ListHandler, RenderHandler};
use crate::tools::{ALL_TOOLS, TOOL_GET, TOOL_LIST, TOOL_RENDER};

pub(super) async fn authenticate_tool_request(
    db_pool: &DbPool,
    tool_name: &str,
    service_id: &str,
    ctx: &RequestContext<RoleServer>,
    authz_hook: &SharedAuthzHook,
) -> Result<SysRequestContext, McpError> {
    match enforce_rbac_from_registry(ctx, service_id, authz_hook).await {
        Ok(result) => {
            match result
                .expect_authenticated("BUG: factsheet requires OAuth but auth was not enforced")
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

#[derive(Debug)]
pub struct Dispatch<'a> {
    pub executor: &'a McpToolExecutor,
    pub request: &'a CallToolRequestParams,
    pub request_context: &'a SysRequestContext,
    pub client: &'a ClientProfile,
}

pub(super) async fn dispatch_tool(
    ctx: &Dispatch<'_>,
    call: Call,
    tool_name: &str,
) -> Result<CallToolResult, McpError> {
    match tool_name {
        TOOL_LIST => ctx.run(&ListHandler { call }).await,
        TOOL_GET => ctx.run(&GetHandler { call }).await,
        TOOL_RENDER => ctx.run(&RenderHandler { call }).await,
        other => Err(unknown_tool(other)),
    }
}

impl Dispatch<'_> {
    async fn run<H: McpToolHandler>(&self, handler: &H) -> Result<CallToolResult, McpError> {
        self.executor
            .execute(handler, self.request, self.request_context, self.client)
            .await
    }
}

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
