//! Entry point for the `email` MCP server binary.

use anyhow::{Context, Result};
use std::env;
use std::sync::Arc;
use systemprompt::config::{ProfileBootstrap, SecretsBootstrap, init_config};
use systemprompt::identifiers::McpServerId;
use systemprompt::system::AppContext;
use systemprompt_mcp_email::EmailServer;
use tokio::net::TcpListener;

const DEFAULT_SERVICE_ID: &str = "email";
const DEFAULT_PORT: u16 = 5050;

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

    // Why: like the knowledge bank, this server is its own process against the
    // tenant database and cannot assume the host binary migrated on its behalf.
    // The DDL is `IF NOT EXISTS` throughout, so this is a no-op after the first
    // boot. Unlike the audit helpers, a failure here is fatal: the outbox is
    // the only record that a real email left the building.
    systemprompt_mcp_email::outbox::ensure_installed(ctx.db_pool())
        .await
        .context("Failed to install the email_outbox schema")?;

    // Why: fail fast rather than at the first send — an operator who has not
    // configured SMTP should find out at boot, not when a human has already
    // approved a message that then cannot go out.
    if let Err(e) = systemprompt_email::EmailService::from_env() {
        tracing::warn!(error = %e, "SMTP is not configured; email_send will draft but refuse to send");
    }

    let server = EmailServer::new(
        Arc::clone(ctx.db_pool()),
        service_id.clone(),
        Arc::clone(ctx.authz_hook()),
    )
    .context("Failed to initialize EmailServer")?;

    let router = systemprompt::mcp::create_router(
        server,
        Arc::clone(ctx.mcp_session_repository()),
        systemprompt::mcp::McpHttpConfig::default(),
    );
    let addr = format!("0.0.0.0:{port}");
    let listener = TcpListener::bind(&addr).await?;

    tracing::info!(service_id = %service_id, addr = %addr, "Email MCP server listening");

    axum::serve(listener, router).await?;

    Ok(())
}
