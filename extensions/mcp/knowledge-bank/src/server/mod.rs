//! The `knowledge-bank` MCP server: struct construction and rmcp
//! `ServerHandler` surface (info, tool listing, call dispatch).
//!
//! Per-call logic (RBAC, the admin gate on uploads, auditing) lives in the
//! `tool` submodule.

#[doc(hidden)]
pub mod tool;

use crate::error::KnowledgeBankError;
use crate::store::KnowledgeStore;
use crate::tools::{self, SERVER_NAME};
use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, InitializeRequestParams,
    InitializeResult, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
    ProtocolVersion, ReadResourceRequestParams, ReadResourceResponse, ServerCapabilities,
    ServerInfo,
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
    build_artifact_viewer_resource, parse_artifact_resource_uri, read_artifact_resource,
    read_artifact_viewer_resource,
};
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt_mcp_shared::record_mcp_access;

use tool::{authenticate_tool_request, dispatch_tool};

#[derive(Clone, Debug)]
pub struct KnowledgeBankServer {
    service_id: McpServerId,
    db_pool: DbPool,
    executor: McpToolExecutor,
    authz_hook: SharedAuthzHook,
    store: KnowledgeStore,
}

impl KnowledgeBankServer {
    pub fn new(
        db_pool: DbPool,
        service_id: McpServerId,
        authz_hook: SharedAuthzHook,
    ) -> Result<Self, KnowledgeBankError> {
        let tool_usage_repo = Arc::new(
            ToolUsageRepository::new(&db_pool)
                .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?,
        );
        let artifact_repo = Arc::new(
            McpArtifactRepository::new(&db_pool)
                .map_err(|e| KnowledgeBankError::Internal(e.to_string()))?,
        );
        let executor = McpToolExecutor::new(tool_usage_repo, artifact_repo, SERVER_NAME);
        let store = KnowledgeStore::new(Arc::clone(&db_pool));

        Ok(Self {
            service_id,
            db_pool,
            executor,
            authz_hook,
            store,
        })
    }
}

impl ServerHandler for KnowledgeBankServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
        )
            .with_protocol_version(ProtocolVersion::V_2024_11_05)
            .with_server_info(
                Implementation::new(
                    format!("Knowledge Bank ({})", self.service_id),
                    env!("CARGO_PKG_VERSION"),
                )
                .with_title("Company Knowledge Bank"),
            )
            .with_instructions(
                "The company knowledge bank: meeting transcripts, documents and notes, \
                 full-text searchable. Search with search_project_context before proposing an \
                 approach; decisions recorded here outrank general best practice. The bank \
                 starts empty and grows only by upload, so an empty result means nothing has \
                 been added yet, not that the search failed. Uploading is admin-only."
                    .to_owned(),
            )
    }

    fn initialize(
        &self,
        _request: InitializeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + MaybeSendFuture + '_ {
        tracing::info!("knowledge-bank MCP server initialized");
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

        record_mcp_access(
            &self.db_pool,
            request_context.user_id(),
            &server_name,
            &tool_name,
            "used",
        )
        .await;

        dispatch_tool(
            &self.executor,
            &self.store,
            &tool_name,
            &request,
            &request_context,
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
            title: "Knowledge Bank Artifact Viewer",
            description: "Interactive UI viewer for knowledge-bank artifacts. Receives the \
                          tool result via the MCP Apps ui/notifications/tool-result protocol \
                          and mounts the server-rendered artifact HTML it carries.",
            template: &artifact_shell_template(),
            icons: None,
        })))
    }

    /// Serves the static shell, plus any `ui://knowledge-bank/artifact/<id>`
    /// the host chooses to resolve instead of using the copy embedded in the
    /// tool result.
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
