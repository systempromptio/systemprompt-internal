//! Spawn the real `systemprompt-mcp-odoo` binary and drive it over the MCP
//! Streamable-HTTP wire protocol with an rmcp client.
//!
//! The subprocess bootstraps from the same fixture profile as the in-process
//! router — same throwaway database, same signing key (written beside the
//! profile), same wiremock Odoo — so a Bearer token minted here validates
//! there, and its tool calls hit the same state the assertions read.

use std::path::PathBuf;
use std::time::Duration;

use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, ClientCapabilities, ClientInfo, Implementation};
use rmcp::transport::streamable_http_client::{
    StreamableHttpClientTransport, StreamableHttpClientTransportConfig,
};
use systemprompt::identifiers::{AgentName, ContextId, SessionId, TraceId};
use systemprompt::mcp::services::client::HttpClientWithContext;
use systemprompt::models::execution::context::RequestContext;

pub struct McpServerProc {
    child: std::process::Child,
    pub port: u16,
}

impl Drop for McpServerProc {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn binary() -> Option<PathBuf> {
    let root = super::stack::profile_path()
        .parent()
        .expect("profile dir has a parent")
        .ancestors()
        .nth(1)
        .expect("tests/target has a parent")
        .to_path_buf();
    let candidates = [
        root.join("target/release/systemprompt-mcp-odoo"),
        root.join("target/debug/systemprompt-mcp-odoo"),
    ];
    let found = candidates.iter().find(|p| p.exists()).cloned();
    assert!(
        !(found.is_none() && std::env::var("CI").is_ok()),
        "systemprompt-mcp-odoo binary missing in CI — build MCP servers before the e2e suite"
    );
    found
}

fn free_port() -> u16 {
    std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind an ephemeral port")
        .local_addr()
        .expect("read the bound address")
        .port()
}

pub async fn spawn_odoo_mcp(odoo_url: &str) -> Option<McpServerProc> {
    let bin = binary()?;
    let port = free_port();
    let child = std::process::Command::new(&bin)
        .env(
            "SYSTEMPROMPT_PROFILE",
            super::stack::profile_path().join("profile.yaml"),
        )
        .env("MCP_PORT", port.to_string())
        .env("MCP_SERVICE_ID", "odoo")
        .env("ODOO_URL", odoo_url)
        .env("ODOO_DB", "e2e_odoo")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn systemprompt-mcp-odoo");
    let proc = McpServerProc { child, port };

    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Some(proc);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!("systemprompt-mcp-odoo never opened port {port} — check the profile it was spawned with");
}

// initialize → tools/call → cancel, over genuine HTTP. Returns the textual
// content blocks joined, which is what the artifact views parse.
pub async fn call_tool(
    port: u16,
    bearer: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    call_tool_at(&format!("http://127.0.0.1:{port}/mcp"), bearer, tool, args).await
}

pub async fn call_tool_at(
    url: &str,
    bearer: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<String, String> {
    let request_context = RequestContext::new(
        SessionId::new(uuid::Uuid::new_v4().to_string()),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("e2e-tests".to_owned()),
    );
    // Why: the transport calls `bearer_auth` on this value, which prepends
    // "Bearer " itself — passing a full header here reaches the server as
    // "Bearer Bearer <jwt>" and fails as a malformed token.
    let config =
        StreamableHttpClientTransportConfig::with_uri(url.to_owned()).auth_header(bearer.to_owned());
    let transport =
        StreamableHttpClientTransport::with_client(HttpClientWithContext::new(request_context), config);
    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new("e2e-tests", "0.0.0"),
    );
    let client = tokio::time::timeout(Duration::from_secs(30), client_info.serve(transport))
        .await
        .map_err(|_| "initialize timed out".to_owned())?
        .map_err(|e| format!("initialize failed: {e}"))?;

    let mut params = CallToolRequestParams::new(tool.to_owned());
    params.arguments = args.as_object().cloned();
    let result = client
        .call_tool(params)
        .await
        .map_err(|e| format!("{tool} rejected: {e}"))?;
    client.cancel().await.map_err(|e| e.to_string())?;

    let text = result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    if result.is_error == Some(true) {
        return Err(format!("{tool} returned an error result: {text}"));
    }
    Ok(text)
}
