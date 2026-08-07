//! The `odoo` MCP server: struct construction and the rmcp `ServerHandler`
//! surface (info, tool listing, call dispatch).
//!
//! Per-call logic — platform authentication, per-user Odoo credential
//! resolution, and routing — lives in the [`tool`] submodule; the handlers
//! themselves in [`crm`], [`report`], [`partner`], [`notes`], [`attachments`],
//! [`activity`], [`calendar`], [`tasks`], [`channels`] and [`overview`].

pub mod activity;
pub mod attachments;
pub mod briefing;
pub mod calendar;
pub mod call;
pub mod channels;
pub mod crm;
mod crm_shape;
pub mod notes;
pub mod overview;
pub mod partner;
pub mod report;
pub mod tasks;
#[doc(hidden)]
pub mod tool;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, InitializeRequestParams,
    InitializeResult, ListToolsResult, PaginatedRequestParams, ProtocolVersion, ServerCapabilities,
    ServerInfo,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler};
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::identifiers::McpServerId;
use systemprompt::mcp::repository::ToolUsageRepository;
use systemprompt::mcp::{McpArtifactRepository, McpToolExecutor};
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::record_mcp_access;

use crate::client::OdooClient;
use crate::error::OdooError;
use crate::tools::{self, SERVER_NAME};
use tool::{authenticate_tool_request, build_call, dispatch_tool};

const INSTRUCTIONS: &str = "Odoo CRM. Every call here runs as *your own* Odoo account, using the \
                            credential you linked on /admin/profile — so what you can see and \
                            change is exactly what Odoo permits you, and your name is on every \
                            note you post and every lead you touch. Prefer \
                            business_overview_data for a daily picture and crm_lead_report for \
                            pipeline numbers; both aggregate in Odoo rather than pulling records \
                            back to count them.";

#[derive(Clone, Debug)]
pub struct OdooServer {
    service_id: McpServerId,
    db_pool: DbPool,
    executor: McpToolExecutor,
    authz_hook: SharedAuthzHook,
    client: Arc<OdooClient>,
}

impl OdooServer {
    /// # Errors
    /// Fails if the repositories cannot be built, or if `ODOO_URL` / `ODOO_DB`
    /// are unset — a server that cannot reach Odoo should refuse to start
    /// rather than accept calls and fail every one of them.
    pub fn new(
        db_pool: DbPool,
        service_id: McpServerId,
        authz_hook: SharedAuthzHook,
    ) -> Result<Self, OdooError> {
        let tool_usage_repo = Arc::new(
            ToolUsageRepository::new(&db_pool).map_err(|e| OdooError::Internal(e.to_string()))?,
        );
        let artifact_repo = Arc::new(
            McpArtifactRepository::new(&db_pool).map_err(|e| OdooError::Internal(e.to_string()))?,
        );
        let executor = McpToolExecutor::new(tool_usage_repo, artifact_repo, SERVER_NAME);
        let client = Arc::new(OdooClient::from_env()?);

        tracing::info!(
            odoo_url = %client.connection().url,
            odoo_db = %client.connection().db,
            "Odoo MCP server configured"
        );

        Ok(Self {
            service_id,
            db_pool,
            executor,
            authz_hook,
            client,
        })
    }
}

impl ServerHandler for OdooServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_server_info(
                Implementation::new(
                    format!("Odoo ({})", self.service_id),
                    env!("CARGO_PKG_VERSION"),
                )
                .with_title("Odoo CRM"),
            )
            .with_instructions(INSTRUCTIONS.to_owned())
    }

    fn initialize(
        &self,
        _request: InitializeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + MaybeSendFuture + '_ {
        tracing::info!("odoo MCP server initialized");
        std::future::ready(Ok(self.get_info()))
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, McpError>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListToolsResult::with_all_items(tools::list_tools())))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        ctx: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, McpError> {
        let tool_name = request.name.to_string();
        let server_name = self.service_id.to_string();

        let request_context = authenticate_tool_request(
            &self.db_pool,
            &tool_name,
            self.service_id.as_str(),
            &ctx,
            &self.authz_hook,
        )
        .await?;

        let call = build_call(&self.db_pool, &self.client, &request_context).await?;

        record_mcp_access(
            &self.db_pool,
            request_context.user_id(),
            &server_name,
            &tool_name,
            "used",
        )
        .await;

        dispatch_tool(&self.executor, call, &tool_name, &request, &request_context)
            .await
            .map(Into::into)
    }
}
