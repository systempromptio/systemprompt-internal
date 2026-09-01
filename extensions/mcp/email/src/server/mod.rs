//! The `email` MCP server: rmcp `ServerHandler` surface.
//!
//! Per-call logic — the MRTR rounds, the approval gate, the send — lives in
//! the `tool` submodule.

pub mod send;
pub mod tool;

use crate::error::EmailToolError;
use crate::tools::{self, SERVER_NAME};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Icon, Implementation, InitializeRequestParams,
    InitializeResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams, ReadResourceResponse,
    ServerCapabilities, ServerInfo,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler};
use std::future::Future;
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::identifiers::McpServerId;
use systemprompt::mcp::repository::ToolUsageRepository;
use systemprompt::mcp::{
    ArtifactViewerConfig, McpArtifactRepository, McpToolExecutor, WEBSITE_URL,
    artifact_shell_template, build_artifact_viewer_resource, build_extension_capabilities,
    build_resource_template_list_result, build_tool_list_result, client_profile_from_peer,
    parse_artifact_resource_uri, read_artifact_resource, read_artifact_viewer_resource,
};
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::record_mcp_access;

use tool::{Dispatch, authenticate_tool_request, dispatch_tool};

#[derive(Clone, Debug)]
pub struct EmailServer {
    service_id: McpServerId,
    db_pool: DbPool,
    executor: McpToolExecutor,
    authz_hook: SharedAuthzHook,
}

impl EmailServer {
    // Why: If either repository cannot be constructed against the pool.
    pub fn new(
        db_pool: DbPool,
        service_id: McpServerId,
        authz_hook: SharedAuthzHook,
    ) -> Result<Self, EmailToolError> {
        let tool_usage_repo = Arc::new(
            ToolUsageRepository::new(&db_pool)
                .map_err(|e| EmailToolError::Internal(e.to_string()))?,
        );
        let artifact_repo = Arc::new(
            McpArtifactRepository::new(&db_pool)
                .map_err(|e| EmailToolError::Internal(e.to_string()))?,
        );
        let executor = McpToolExecutor::new(tool_usage_repo, artifact_repo, SERVER_NAME);

        Ok(Self {
            service_id,
            db_pool,
            executor,
            authz_hook,
        })
    }
}

impl ServerHandler for EmailServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_extensions_with(build_extension_capabilities())
                .build(),
        )
        // Why: Load-bearing, not housekeeping: rmcp refuses to let an
        // `InputRequiredResult` reach a peer below 2026-07-28, and both of this
        // server's rounds — the confirm elicitation and the approval hold —
        // are exactly that.
        .with_protocol_version(ProtocolVersion::V_2026_07_28)
        .with_server_info(
            Implementation::new(
                format!("SystemPrompt Email ({})", self.service_id),
                env!("CARGO_PKG_VERSION"),
            )
            .with_title("SystemPrompt Email")
            .with_icons(vec![
                Icon::new(format!("{WEBSITE_URL}/files/images/favicon-32x32.png"))
                    .with_mime_type("image/png")
                    .with_sizes(vec!["32x32".to_owned()]),
            ])
            .with_website_url(WEBSITE_URL),
        )
        .with_instructions(
            "Send email. `email_send` never sends on the first call: it returns a draft preview \
             and a confirmation request for a human to answer, and depending on the recipient and \
             the caller's role a second human may have to approve it as well. Present the draft to \
             the user; do not confirm on their behalf. Pass res_model and res_id together to log \
             the sent mail on an Odoo record."
                .to_owned(),
        )
    }

    fn initialize(
        &self,
        _request: InitializeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + MaybeSendFuture + '_ {
        tracing::info!("email MCP server initialized");
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
            &Dispatch {
                db_pool: &self.db_pool,
                executor: &self.executor,
                request: &request,
                request_context: &request_context,
                client: &client,
            },
            &tool_name,
        )
        .await
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, McpError>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(build_artifact_viewer_resource(&ArtifactViewerConfig {
            server_name: SERVER_NAME,
            title: "systemprompt.io Email Artifact Viewer",
            description: "Interactive UI viewer for email draft previews and send receipts.",
            template: &artifact_shell_template(),
            icons: Some(vec![
                Icon::new(format!("{WEBSITE_URL}/files/images/favicon-32x32.png"))
                    .with_mime_type("image/png")
                    .with_sizes(vec!["32x32".to_owned()]),
            ]),
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
