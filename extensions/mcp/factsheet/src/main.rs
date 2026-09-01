//! Entry point for the `factsheet` MCP server binary.

use anyhow::{Context, Result};
use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use systemprompt::config::{ProfileBootstrap, SecretsBootstrap, init_config};
use systemprompt::files::FilesConfig;
use systemprompt::identifiers::McpServerId;
use systemprompt::system::AppContext;
use systemprompt_factsheet::EnginePaths;
use systemprompt_mcp_factsheet::FactsheetServer;
use systemprompt_mcp_factsheet::server::ServerConfig;
use tokio::net::TcpListener;

const DEFAULT_SERVICE_ID: &str = "factsheet";
const DEFAULT_PORT: u16 = 5070;

// Why: Interpreter for the `WeasyPrint` sidecar. A deployed image points this
// at the venv that has `WeasyPrint` installed; locally the system `python3`
// will do.
const PYTHON_ENV: &str = "FACTSHEET_PYTHON";
const DEFAULT_PYTHON: &str = "python3";

#[tokio::main]
async fn main() -> Result<()> {
    systemprompt::logging::init_console_logging();

    ProfileBootstrap::init().context("Failed to initialize profile")?;
    SecretsBootstrap::init().context("Failed to initialize secrets")?;
    init_config().context("Failed to initialize configuration")?;

    let ctx = Arc::new(
        AppContext::new()
            .await
            .context("Failed to initialize application context")?,
    );

    let service_id = env::var("MCP_SERVICE_ID").map_or_else(
        |_| {
            tracing::warn!(
                default = DEFAULT_SERVICE_ID,
                "MCP_SERVICE_ID not set, using default"
            );
            McpServerId::new(DEFAULT_SERVICE_ID)
        },
        McpServerId::new,
    );

    let port = env::var("MCP_PORT").map_or_else(
        |_| {
            tracing::warn!(default = DEFAULT_PORT, "MCP_PORT not set, using default");
            DEFAULT_PORT
        },
        |p| {
            p.parse::<u16>().unwrap_or_else(|e| {
                tracing::warn!(error = %e, port = %p, default = DEFAULT_PORT, "Invalid MCP_PORT, using default");
                DEFAULT_PORT
            })
        },
    );

    let app_paths = ctx.app_paths();
    let files_config = FilesConfig::get().map_or_else(
        |_| FilesConfig::from_profile(app_paths),
        |config| Ok(config.clone()),
    )?;

    let engine_paths = EnginePaths {
        root: app_paths.storage().files().join("factsheet"),
        script: app_paths
            .system()
            .root()
            .join("scripts/factsheet-render.py"),
        python: env::var(PYTHON_ENV).map_or_else(|_| PathBuf::from(DEFAULT_PYTHON), PathBuf::from),
    };
    let work_dir = env::temp_dir().join("systemprompt-factsheet");

    let server = FactsheetServer::new(
        Arc::clone(ctx.db_pool()),
        Arc::clone(ctx.authz_hook()),
        ServerConfig {
            service_id: service_id.clone(),
            paths: engine_paths,
            files_config,
            work_dir,
        },
    )
    .context("Failed to initialize FactsheetServer")?;

    let router = systemprompt::mcp::create_router(
        server,
        Arc::clone(ctx.mcp_session_repository()),
        systemprompt::mcp::McpHttpConfig::default(),
    );
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!(service_id = %service_id, addr = %addr, "Factsheet MCP server listening");

    axum::serve(listener, router).await?;
    Ok(())
}
