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

// Why newest-by-mtime rather than release-then-debug: this used to prefer
// `target/release/` and fall back to debug, which silently spawned whatever
// release build happened to be lying around — for one afternoon, a binary
// predating `extensions/mcp/shared/src/artifact_theme.rs` entirely. The
// artifact assertions then failed saying the theme registration was broken,
// when the truth was that the suite was testing an older build. Picking the
// most recently built binary makes the local loop (`just build` produces
// debug) do the obvious thing, and CI builds exactly one of the two anyway.
fn newest_binary(name: &str) -> Option<PathBuf> {
    let root = super::stack::profile_path()
        .parent()
        .expect("profile dir has a parent")
        .ancestors()
        .nth(2)
        .expect("tests/target sits two levels below the repository root")
        .to_path_buf();
    let found = [
        root.join(format!("target/release/{name}")),
        root.join(format!("target/debug/{name}")),
    ]
    .into_iter()
    .filter(|p| p.exists())
    .max_by_key(|p| {
        std::fs::metadata(p)
            .and_then(|m| m.modified())
            .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
    });
    assert!(
        !(found.is_none() && std::env::var("CI").is_ok()),
        "{name} binary missing in CI — build MCP servers before the e2e suite"
    );
    found
}

fn binary() -> Option<PathBuf> {
    newest_binary("systemprompt-mcp-odoo")
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
        .stdout(log_sink())
        .stderr(log_sink())
        .spawn()
        .expect("spawn systemprompt-mcp-odoo");
    let proc = McpServerProc { child, port };

    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Some(proc);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!(
        "systemprompt-mcp-odoo never opened port {port} — check the profile it was spawned with"
    );
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
    call_tool_full(url, bearer, tool, args).await.map(|r| r.0)
}

// The textual blocks joined, plus the structured content — the machine half
// of the contract the dashboards consume.
pub async fn call_tool_full(
    url: &str,
    bearer: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<(String, Option<serde_json::Value>), String> {
    let result = raw_call(url, bearer, tool, args).await?;
    let text = joined_text(&result);
    if result.is_error == Some(true) {
        return Err(format!("{tool} returned an error result: {text}"));
    }
    let structured = result
        .structured_content
        .clone()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| e.to_string())?;
    Ok((text, structured))
}

fn joined_text(result: &rmcp::model::CallToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n")
}

// initialize → tools/call → cancel, handing back the whole result so each
// caller can take the half of it that it cares about.
async fn raw_call(
    url: &str,
    bearer: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<rmcp::model::CallToolResult, String> {
    raw_call_as(url, bearer, tool, args, false).await
}

async fn raw_call_as(
    url: &str,
    bearer: &str,
    tool: &str,
    args: serde_json::Value,
    ui_capable: bool,
) -> Result<rmcp::model::CallToolResult, String> {
    let request_context = RequestContext::new(
        SessionId::new(uuid::Uuid::new_v4().to_string()),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("e2e-tests".to_owned()),
    );
    // Why: the transport calls `bearer_auth` on this value, which prepends
    // "Bearer " itself — passing a full header here reaches the server as
    // "Bearer Bearer <jwt>" and fails as a malformed token.
    let capabilities = client_capabilities_for(ui_capable);
    let config = StreamableHttpClientTransportConfig::with_uri(url.to_owned())
        .auth_header(bearer.to_owned());
    // Why: under 2026-07-28 (SEP-2575) the server reads the client's
    // capabilities from every request's `_meta`, not from `initialize`, and
    // core's `HttpClientWithContext` stamps that `_meta` with whatever it is
    // given. Handing it a different set than the one declared at initialize
    // made the server see a client without the UI extension, so no tool ever
    // embedded its artifact — the exact thing `call_tool_resource` asserts.
    let transport = StreamableHttpClientTransport::with_client(
        HttpClientWithContext::new(request_context).with_client_capabilities(capabilities.clone()),
        config,
    );
    // Why: every MCP server here advertises 2026-07-28, and rmcp refuses to
    // hand an `InputRequiredResult` to a peer that negotiated below it — so a
    // tool held by the `require_approval` stage (note_add, channel_post,
    // email_send) comes back as "-32600: InputRequiredResult requires
    // negotiated protocol version 2026-07-28 or newer" rather than as the hold
    // it actually is. The default is older, so say it explicitly; the sibling
    // client in `McpSession::connect` has always done this.
    let client_info = ClientInfo::new(capabilities, Implementation::new("e2e-tests", "0.0.0"))
        .with_protocol_version(rmcp::model::ProtocolVersion::V_2026_07_28);
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

    if result.is_error == Some(true) {
        return Err(format!(
            "{tool} returned an error result: {}",
            joined_text(&result)
        ));
    }
    Ok(result)
}

// Why: the server only embeds the rendered artifact when the client declared
// the `io.modelcontextprotocol/ui` extension at initialize (SEP-1724) —
// `ClientProfile::supports_ui`. A client that does not ask for UI gets the text
// summary alone, which is correct behaviour and means a test using
// `ClientCapabilities::default()` can never see an artifact. This is the
// capability set Cowork sends.
fn client_capabilities_for(ui_capable: bool) -> ClientCapabilities {
    let mut capabilities = ClientCapabilities::default();
    if ui_capable {
        let mut extensions = rmcp::model::ExtensionCapabilities::new();
        extensions.insert(
            "io.modelcontextprotocol/ui".to_owned(),
            serde_json::Map::new(),
        );
        capabilities.extensions = Some(extensions);
    }
    capabilities
}

// The embedded UI resource — `ui://` URI, mime, and HTML — which is what
// Cowork renders. `call_tool_full` above returns only the text and structured
// blocks, so nothing there can see whether the artifact came back branded, or
// came back at all.
pub struct EmbeddedUi {
    pub uri: String,
    pub mime_type: Option<String>,
    pub html: String,
}

pub async fn call_tool_resource(
    url: &str,
    bearer: &str,
    tool: &str,
    args: serde_json::Value,
) -> Result<EmbeddedUi, String> {
    let result = raw_call_as(url, bearer, tool, args, true).await?;
    result
        .content
        .iter()
        .find_map(|block| match block {
            rmcp::model::ContentBlock::Resource(embedded) => match &embedded.resource {
                rmcp::model::ResourceContents::TextResourceContents {
                    uri,
                    mime_type,
                    text,
                    ..
                } => Some(EmbeddedUi {
                    uri: uri.clone(),
                    mime_type: mime_type.clone(),
                    html: text.clone(),
                }),
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| {
            // Why: the block kinds and `_meta` say whether the server took the
            // client for UI-capable at all, which a bare "no resource" cannot.
            let kinds: Vec<String> = result
                .content
                .iter()
                .map(|b| {
                    serde_json::to_value(b)
                        .ok()
                        .and_then(|v| v.get("type").and_then(|t| t.as_str()).map(str::to_owned))
                        .unwrap_or_else(|| "?".to_owned())
                })
                .collect();
            let meta = serde_json::to_string(&result.meta).unwrap_or_default();
            format!(
                "{tool} returned no embedded text resource; content kinds {kinds:?}, _meta {meta}"
            )
        })
}

// Why a second spawn fn rather than a parameter on the first: the two servers
// take different env. Odoo needs ODOO_URL/ODOO_DB; email takes its SMTP
// settings from the fixture profile's secrets, so the only thing it needs told
// is which service it is.
pub async fn spawn_email_mcp() -> Option<McpServerProc> {
    let bin = email_binary()?;
    let port = free_port();
    let child = std::process::Command::new(&bin)
        .env(
            "SYSTEMPROMPT_PROFILE",
            super::stack::profile_path().join("profile.yaml"),
        )
        .env("MCP_PORT", port.to_string())
        .env("MCP_SERVICE_ID", "email")
        .stdout(log_sink())
        .stderr(log_sink())
        .spawn()
        .expect("spawn systemprompt-mcp-email");
    let proc = McpServerProc { child, port };

    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Some(proc);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!(
        "systemprompt-mcp-email never opened port {port} — check the profile it was spawned with"
    )
}

fn email_binary() -> Option<PathBuf> {
    newest_binary("systemprompt-mcp-email")
}

// The admin CLI-passthrough server: the profile tells it where the
// `systemprompt` binary lives, so it needs no service-specific env beyond its
// id.
pub async fn spawn_systemprompt_mcp() -> Option<McpServerProc> {
    let bin = newest_binary("systemprompt-mcp-agent")?;
    let port = free_port();
    let child = std::process::Command::new(&bin)
        .env(
            "SYSTEMPROMPT_PROFILE",
            super::stack::profile_path().join("profile.yaml"),
        )
        .env("MCP_PORT", port.to_string())
        .env("MCP_SERVICE_ID", "systemprompt")
        .stdout(log_sink())
        .stderr(log_sink())
        .spawn()
        .expect("spawn systemprompt-mcp-agent");
    let proc = McpServerProc { child, port };

    for _ in 0..100 {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return Some(proc);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    panic!(
        "systemprompt-mcp-agent never opened port {port} — check the profile it was spawned with"
    )
}

// initialize → tools/list → cancel; the tool names and their annotations.
pub async fn list_tools(port: u16, bearer: &str) -> Result<Vec<rmcp::model::Tool>, String> {
    let request_context = RequestContext::new(
        SessionId::new(uuid::Uuid::new_v4().to_string()),
        TraceId::generate(),
        ContextId::generate(),
        AgentName::new("e2e-tests".to_owned()),
    );
    let config =
        StreamableHttpClientTransportConfig::with_uri(format!("http://127.0.0.1:{port}/mcp"))
            .auth_header(bearer.to_owned());
    let transport = StreamableHttpClientTransport::with_client(
        HttpClientWithContext::new(request_context),
        config,
    );
    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new("e2e-tests", "0.0.0"),
    );
    let client = tokio::time::timeout(Duration::from_secs(30), client_info.serve(transport))
        .await
        .map_err(|_| "initialize timed out".to_owned())?
        .map_err(|e| format!("initialize failed: {e}"))?;
    let tools = client
        .list_all_tools()
        .await
        .map_err(|e| format!("tools/list failed: {e}"))?;
    let _ = client.cancel().await;
    Ok(tools)
}

// An MCP client that stays connected across MRTR rounds.
//
// `call_tool_full` above cannot be used for a tool that answers
// `input_required`: rmcp's `call_tool` drives the MRTR loop internally, and
// with no elicitation delegate installed it answers the server's request by
// declining. That is the correct default for a client that cannot ask a human,
// but it means the confirm round can never be observed, let alone answered.
// `call_tool_once` issues exactly one round and hands back the raw response, so
// a test can inspect what the server asked and reply on the retry — which is
// the whole point of the flow under test.
pub struct MrtrClient {
    client: rmcp::service::RunningService<rmcp::service::RoleClient, ClientInfo>,
}

impl MrtrClient {
    pub async fn connect(port: u16, bearer: &str) -> Result<Self, String> {
        let request_context = RequestContext::new(
            SessionId::new(uuid::Uuid::new_v4().to_string()),
            TraceId::generate(),
            ContextId::generate(),
            AgentName::new("e2e-tests".to_owned()),
        );
        let config =
            StreamableHttpClientTransportConfig::with_uri(format!("http://127.0.0.1:{port}/mcp"))
                .auth_header(bearer.to_owned());
        // Why: core's `HttpClientWithContext` stamps the SEP-2575 `_meta` that a
        // 2026-07-28 server requires, and that is also what makes rmcp derive
        // the `Mcp-Method` / `Mcp-Name` headers. Nothing test-specific needed.
        let transport = StreamableHttpClientTransport::with_client(
            HttpClientWithContext::new(request_context)
                .with_client_capabilities(ClientCapabilities::default()),
            config,
        );
        // Why: rmcp refuses to hand an `InputRequiredResult` to a peer that
        // negotiated below 2026-07-28, and the default is older. Without this
        // the server answers "-32600: InputRequiredResult requires negotiated
        // protocol version 2026-07-28 or newer" and the confirm round can never
        // be observed.
        let client_info = ClientInfo::new(
            ClientCapabilities::default(),
            Implementation::new("e2e-tests", "0.0.0"),
        )
        .with_protocol_version(rmcp::model::ProtocolVersion::V_2026_07_28);
        let client = tokio::time::timeout(Duration::from_secs(30), client_info.serve(transport))
            .await
            .map_err(|_| "initialize timed out".to_owned())?
            .map_err(|e| format!("initialize failed: {e}"))?;
        Ok(Self { client })
    }

    // One round, raw. `params` carries whatever `input_responses` /
    // `request_state` the caller is answering with.
    pub async fn call_once(
        &self,
        params: CallToolRequestParams,
    ) -> Result<rmcp::model::CallToolResponse, String> {
        self.client
            .call_tool_once(params)
            .await
            .map_err(|e| format!("tools/call failed: {e}"))
    }

    pub async fn cancel(self) {
        let _ = self.client.cancel().await;
    }
}

// The arguments half of a call, without any MRTR answer attached.
pub fn call_params(tool: &str, args: serde_json::Value) -> CallToolRequestParams {
    let mut params = CallToolRequestParams::new(tool.to_owned());
    params.arguments = args.as_object().cloned();
    params
}

// Answer a confirm-round elicitation, in the shape a real client would send.
pub fn with_confirmation(
    mut params: CallToolRequestParams,
    key: &str,
    accept: bool,
    confirm: bool,
) -> CallToolRequestParams {
    let action = if accept { "accept" } else { "decline" };
    let mut responses = rmcp::model::InputResponses::new();
    responses.insert(
        key.to_owned(),
        serde_json::json!({ "action": action, "content": { "confirm": confirm } }),
    );
    params.input_responses = Some(responses);
    params
}

// Why: the subprocess is silent by default, which makes a server-side refusal
// arrive at the test as a bare HTTP status with no cause. Setting
// E2E_MCP_LOG=<path> tees its output somewhere readable.
fn log_sink() -> std::process::Stdio {
    std::env::var("E2E_MCP_LOG")
        .ok()
        .map_or_else(std::process::Stdio::null, |path| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_or_else(|_| std::process::Stdio::null(), std::process::Stdio::from)
        })
}
