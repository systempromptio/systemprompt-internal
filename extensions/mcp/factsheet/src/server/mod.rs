//! The `factsheet` MCP server.

pub mod handlers;
#[doc(hidden)]
pub mod tool;

use rmcp::model::{
    CallToolRequestParams, CallToolResponse, Implementation, InitializeRequestParams,
    InitializeResult, ListResourceTemplatesResult, ListResourcesResult, ListToolsResult,
    PaginatedRequestParams, ProtocolVersion, ReadResourceRequestParams, ReadResourceResponse,
    ServerCapabilities, ServerInfo,
};
use rmcp::service::{MaybeSendFuture, RequestContext, RoleServer};
use rmcp::{ErrorData as McpError, ServerHandler};
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::files::FilesConfig;
use systemprompt::identifiers::McpServerId;
use systemprompt::mcp::repository::ToolUsageRepository;
use systemprompt::mcp::{
    ArtifactViewerConfig, McpArtifactRepository, McpToolExecutor, artifact_shell_template,
    build_artifact_viewer_resource, build_extension_capabilities,
    build_resource_template_list_result, build_tool_list_result, client_profile_from_peer,
    parse_artifact_resource_uri, read_artifact_resource, read_artifact_viewer_resource,
};
use systemprompt::security::authz::SharedAuthzHook;
use systemprompt::traits::FileStorage;
use systemprompt_factsheet::{EnginePaths, FactsheetEngine};
use systemprompt_mcp_shared::approval::{GateOutcome, enforce_approval};
use systemprompt_mcp_shared::record_mcp_access;

use crate::error::ServerError;
use crate::tools::{self, SERVER_NAME};
use handlers::Call;
use tool::{authenticate_tool_request, dispatch_tool};

const INSTRUCTIONS: &str = "Factsheets. A factsheet here is data, not a document: one template \
                            and one design system, rendered from a typed content model. Read a \
                            sheet with factsheet_get, change the blocks that should differ, and \
                            render it with factsheet_render — that is also how you build a sheet \
                            for a specific customer or lead. The house style is a two-page \
                            document and the renderer enforces it, so keep the copy tight.";

#[derive(Clone)]
pub struct FactsheetServer {
    service_id: McpServerId,
    db_pool: DbPool,
    executor: McpToolExecutor,
    authz_hook: SharedAuthzHook,
    engine: Arc<FactsheetEngine>,
    files_config: Arc<FilesConfig>,
    storage: Arc<dyn FileStorage>,
    work_dir: Arc<PathBuf>,
}

/// Everything the server needs that is not the database or the authorization
/// hook: which service it is, where the engine's inputs live, and where its
/// output goes.
#[derive(Clone)]
pub struct ServerConfig {
    pub service_id: McpServerId,
    pub paths: EnginePaths,
    pub files_config: FilesConfig,
    pub storage: Arc<dyn FileStorage>,
    pub work_dir: PathBuf,
}

impl std::fmt::Debug for FactsheetServer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FactsheetServer")
            .field("service_id", &self.service_id)
            .field("work_dir", &self.work_dir)
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for ServerConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServerConfig")
            .field("service_id", &self.service_id)
            .field("paths", &self.paths)
            .field("work_dir", &self.work_dir)
            .finish_non_exhaustive()
    }
}

impl FactsheetServer {
    pub fn new(
        db_pool: DbPool,
        authz_hook: SharedAuthzHook,
        config: ServerConfig,
    ) -> Result<Self, ServerError> {
        let ServerConfig {
            service_id,
            paths,
            files_config,
            storage,
            work_dir,
        } = config;
        let tool_usage_repo = Arc::new(
            ToolUsageRepository::new(&db_pool).map_err(|e| ServerError::Internal(e.to_string()))?,
        );
        let artifact_repo = Arc::new(
            McpArtifactRepository::new(&db_pool)
                .map_err(|e| ServerError::Internal(e.to_string()))?,
        );
        let executor = McpToolExecutor::new(tool_usage_repo, artifact_repo, SERVER_NAME);

        tracing::info!(
            root = %paths.root.display(),
            script = %paths.script.display(),
            python = %paths.python.display(),
            "Factsheet MCP server configured"
        );
        let engine = Arc::new(FactsheetEngine::new(paths)?);

        Ok(Self {
            service_id,
            db_pool,
            executor,
            authz_hook,
            engine,
            files_config: Arc::new(files_config),
            storage,
            work_dir: Arc::new(work_dir),
        })
    }

    fn call(&self) -> Call {
        Call {
            engine: Arc::clone(&self.engine),
            db_pool: Arc::clone(&self.db_pool),
            files_config: Arc::clone(&self.files_config),
            storage: Arc::clone(&self.storage),
            work_dir: Arc::clone(&self.work_dir),
        }
    }
}

impl ServerHandler for FactsheetServer {
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
                format!("Factsheet ({})", self.service_id),
                env!("CARGO_PKG_VERSION"),
            )
            .with_title("Factsheets"),
        )
        .with_instructions(INSTRUCTIONS.to_owned())
    }

    fn initialize(
        &self,
        _request: InitializeRequestParams,
        _ctx: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<InitializeResult, McpError>> + MaybeSendFuture + '_ {
        tracing::info!("factsheet MCP server initialized");
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
            GateOutcome::Held(result) => return Ok(CallToolResponse::InputRequired(*result)),
            GateOutcome::Refused(result) => return Ok((*result).into()),
        }

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
            self.call(),
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
            title: "Factsheet Artifact Viewer",
            description: "Interactive UI viewer for factsheet artifacts, including rendered page \
                          previews.",
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
