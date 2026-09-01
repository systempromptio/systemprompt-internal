//! The `odoo` MCP server: struct construction and the rmcp `ServerHandler`
//! surface (info, tool listing, call dispatch).
//!
//! Per-call logic — platform authentication, per-user Odoo credential
//! resolution, and routing — lives in the [`tool`] submodule; the handlers
//! themselves in [`crm`], [`crm_delete`], [`report`], [`partner`], [`notes`],
//! [`attachments`], [`activity`], [`calendar`], [`tasks`], [`channels`] and
//! [`overview`].

pub mod activity;
pub mod attachments;
pub mod briefing;
pub mod calendar;
pub mod call;
pub mod channels;
pub mod crm;
pub mod crm_delete;
mod crm_shape;
pub mod notes;
pub mod overview;
pub mod partner;
pub mod report;
pub mod tasks;
#[doc(hidden)]
pub mod tool;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, CallToolResult, ContentBlock, Implementation,
    InitializeRequestParams, InitializeResult, ListResourceTemplatesResult, ListResourcesResult,
    ListToolsResult, PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams,
    ReadResourceResponse, ServerCapabilities, ServerInfo,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler};
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::identifiers::McpServerId;
use systemprompt::mcp::repository::ToolUsageRepository;
use systemprompt::mcp::{
    ArtifactViewerConfig, McpArtifactRepository, McpToolExecutor, artifact_shell_template,
    build_artifact_viewer_resource, build_extension_capabilities,
    build_resource_template_list_result, build_tool_list_result, client_profile_from_peer,
    parse_artifact_resource_uri, read_artifact_resource, read_artifact_viewer_resource,
};
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::approval::{GateOutcome, enforce_approval};
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
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_extensions_with(build_extension_capabilities())
                .build(),
        )
        .with_protocol_version(ProtocolVersion::V_2026_07_28)
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
        std::future::ready(Ok(build_tool_list_result(tools::list_tools())))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, McpError>> + MaybeSendFuture + '_
    {
        std::future::ready(Ok(build_resource_template_list_result()))
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

        // Why: ahead of build_call — a held call must not resolve an Odoo
        // credential or touch Odoo at all until a human has authorised it.
        match enforce_approval(
            &self.db_pool,
            &server_name,
            &tool_name,
            &request,
            &request_context,
        )
        .await
        {
            GateOutcome::Proceed => {},
            GateOutcome::Held(result) => {
                return Ok(CallToolResponse::InputRequired(*result));
            },
            GateOutcome::Refused(result) => return Ok((*result).into()),
        }

        let call = match build_call(&self.db_pool, &self.client, &request_context).await {
            Ok(call) => call,
            // Why: link/setup states must reach the UI as isError results —
            // a JSON-RPC error is rejected by strict bridges before rendering.
            Err(
                err @ (OdooError::NotLinked(_)
                | OdooError::NotConfigured(_)
                | OdooError::AppMissing(_)),
            ) => {
                return Ok(CallToolResult::error(vec![ContentBlock::text(err.to_string())]).into());
            },
            Err(other) => return Err(other.into()),
        };

        record_mcp_access(
            &self.db_pool,
            request_context.user_id(),
            &server_name,
            &tool_name,
            "used",
        )
        .await;

        let client = client_profile_from_peer(&ctx);
        dispatch_tool(
            &tool::Dispatch {
                executor: &self.executor,
                request: &request,
                request_context: &request_context,
                client: &client,
            },
            call,
            &tool_name,
        )
        .await
        .map(Into::into)
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(build_artifact_viewer_resource(&ArtifactViewerConfig {
            server_name: SERVER_NAME,
            title: "Odoo Artifact Viewer",
            description: "Interactive UI viewer for Odoo artifacts. Receives the tool result \
                          via the MCP Apps ui/notifications/tool-result protocol and mounts \
                          the server-rendered artifact HTML it carries.",
            template: &artifact_shell_template(),
            icons: None,
        })))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResponse, McpError> {
        if parse_artifact_resource_uri(&request.uri).is_some() {
            let repo = McpArtifactRepository::new(&self.db_pool)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;
            return read_artifact_resource(&request, SERVER_NAME, &repo)
                .await
                .map(Into::into);
        }

        read_artifact_viewer_resource(&request, SERVER_NAME, &artifact_shell_template())
            .map(Into::into)
    }
}
