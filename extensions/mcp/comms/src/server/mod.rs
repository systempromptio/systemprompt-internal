//! The `comms` MCP server: struct construction and rmcp
//! `ServerHandler` surface (info, tool listing, call dispatch).
//!
//! Per-call logic (RBAC, the admin gate on uploads, auditing) lives in the
//! `tool` submodule.

#[doc(hidden)]
pub mod tool;

use crate::error::CommsError;
use crate::store::CommsStore;
use crate::tools::{self, SERVER_NAME};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, InitializeRequestParams,
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
    ArtifactViewerConfig, McpArtifactRepository, McpToolExecutor, artifact_shell_template,
    build_artifact_viewer_resource, build_extension_capabilities,
    build_resource_template_list_result, build_tool_list_result, client_profile_from_peer,
    parse_artifact_resource_uri, read_artifact_resource, read_artifact_viewer_resource,
};
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::record_mcp_access;

use tool::{authenticate_tool_request, dispatch_tool};

#[derive(Clone, Debug)]
pub struct CommsServer {
    service_id: McpServerId,
    db_pool: DbPool,
    executor: McpToolExecutor,
    authz_hook: SharedAuthzHook,
    store: CommsStore,
}

impl CommsServer {
    pub fn new(
        db_pool: DbPool,
        service_id: McpServerId,
        authz_hook: SharedAuthzHook,
    ) -> Result<Self, CommsError> {
        let tool_usage_repo = Arc::new(
            ToolUsageRepository::new(&db_pool).map_err(|e| CommsError::Internal(e.to_string()))?,
        );
        let artifact_repo = Arc::new(
            McpArtifactRepository::new(&db_pool)
                .map_err(|e| CommsError::Internal(e.to_string()))?,
        );
        let executor = McpToolExecutor::new(tool_usage_repo, artifact_repo, SERVER_NAME);
        let store = CommsStore::new(Arc::clone(&db_pool));

        Ok(Self {
            service_id,
            db_pool,
            executor,
            authz_hook,
            store,
        })
    }
}

impl ServerHandler for CommsServer {
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
                format!("Team Comms ({})", self.service_id),
                env!("CARGO_PKG_VERSION"),
            )
            .with_title("Team Comms"),
        )
        .with_instructions(
            "Team comms: messages between people and their agent sessions. Read yours with \
                 comms_inbox; it returns only what was addressed to you or to this session, and \
                 each session keeps its own unread mark. Send with comms_send: @user reaches a \
                 person's inbox and never interrupts them, @user/session-handle reaches one \
                 running session, #channel reaches a channel. Find handles with comms_sessions."
                .to_owned(),
        )
    }

    fn initialize(
        &self,
        _request: InitializeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + MaybeSendFuture + '_ {
        tracing::info!("comms MCP server initialized");
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
            &tool::Dispatch {
                executor: &self.executor,
                request: &request,
                request_context: &request_context,
                client: &client,
            },
            &self.store,
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
            title: "Team Comms Artifact Viewer",
            description: "Interactive UI viewer for comms artifacts. Receives the \
                          tool result via the MCP Apps ui/notifications/tool-result protocol \
                          and mounts the server-rendered artifact HTML it carries.",
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
