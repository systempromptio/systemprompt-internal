//! Per-call logic for the `comms` server: authentication and dispatch.
//!
//! The policy chain is deliberately not invoked here. Agent tool calls are
//! already governed at the hook plane (`POST /hooks/govern`) before they
//! reach any MCP server, and `GovernanceEngine::global()` is a process
//! singleton — a second evaluation would double-count the shared rate limiter.

pub mod common;
pub mod fanout;
pub mod reads;
pub mod render;
pub mod send;

use crate::store::CommsStore;
use crate::tools::{TOOL_CHANNELS, TOOL_HISTORY, TOOL_INBOX, TOOL_SEND, TOOL_SESSIONS};
use rmcp::ErrorData as McpError;
use rmcp::model::{CallToolRequestParams, CallToolResult};
use rmcp::service::{RequestContext, RoleServer};
use systemprompt::database::DbPool;
use systemprompt::mcp::middleware::enforce_rbac_from_registry;
use systemprompt::mcp::{ClientProfile, McpToolExecutor};
use systemprompt::models::execution::context::RequestContext as SysRequestContext;
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::{record_mcp_access, record_mcp_access_rejected};

use reads::{ChannelsHandler, HistoryHandler, InboxHandler, SessionsHandler};
use send::SendHandler;

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
    match enforce_rbac_from_registry(ctx, service_id, authz_hook).await {
        Ok(result) => {
            match result.expect_authenticated("BUG: comms requires OAuth but auth was not enforced")
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

#[doc(hidden)]
pub async fn dispatch_tool(
    ctx: &Dispatch<'_>,
    store: &CommsStore,
    tool_name: &str,
) -> Result<CallToolResult, McpError> {
    match tool_name {
        TOOL_SEND => {
            ctx.run(&SendHandler {
                store: store.clone(),
            })
            .await
        },
        TOOL_INBOX => {
            ctx.run(&InboxHandler {
                store: store.clone(),
            })
            .await
        },
        TOOL_HISTORY => {
            ctx.run(&HistoryHandler {
                store: store.clone(),
            })
            .await
        },
        TOOL_CHANNELS => {
            ctx.run(&ChannelsHandler {
                store: store.clone(),
            })
            .await
        },
        TOOL_SESSIONS => {
            ctx.run(&SessionsHandler {
                store: store.clone(),
            })
            .await
        },
        _ => Err(McpError::invalid_params(
            format!(
                "Unknown tool: '{tool_name}'. Available tools: {TOOL_SEND}, {TOOL_INBOX}, \
                 {TOOL_HISTORY}, {TOOL_CHANNELS}, {TOOL_SESSIONS}."
            ),
            None,
        )),
    }
}
