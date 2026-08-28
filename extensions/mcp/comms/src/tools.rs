//! Tool definitions exposed by the `comms` MCP server.
//!
//! Addresses are written the way people say them — `@ed`, `@ed/odoo-crm`,
//! `#crm` — and the address form decides whether a message may interrupt a
//! running conversation. Only a session address can.

use rmcp::model::{MetaObject, Tool, ToolAnnotations};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt::mcp::{McpOutputSchema, default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::CliArtifact;

pub const SERVER_NAME: &str = "comms";
pub const TOOL_SEND: &str = "comms_send";
pub const TOOL_INBOX: &str = "comms_inbox";
pub const TOOL_HISTORY: &str = "comms_history";
pub const TOOL_CHANNELS: &str = "comms_channels";
pub const TOOL_SESSIONS: &str = "comms_sessions";
pub const TOOL_WHOAMI: &str = "comms_whoami";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SendInput {
    pub to: String,
    pub body: String,
    pub thread_id: Option<String>,
    pub urgent: Option<bool>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct InboxInput {
    pub limit: Option<u32>,
    pub peek: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HistoryInput {
    pub with: String,
    pub limit: Option<u32>,
}

// Why: braces rather than a unit struct because an MCP client sends `{}` for a
// no-argument tool, and serde deserializes a unit struct only from `null`.
#[expect(
    clippy::empty_structs_with_brackets,
    reason = "must deserialize from the empty JSON object an MCP client sends"
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct ChannelsInput {}

#[expect(
    clippy::empty_structs_with_brackets,
    reason = "must deserialize from the empty JSON object an MCP client sends"
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct SessionsInput {}

#[expect(
    clippy::empty_structs_with_brackets,
    reason = "must deserialize from the empty JSON object an MCP client sends"
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct WhoamiInput {}

struct ToolDef<'a> {
    name: &'a str,
    title: &'a str,
    description: &'a str,
    // JSON: protocol boundary
    input_schema: serde_json::Value,
    read_only: bool,
}

fn create_tool(def: &ToolDef<'_>) -> Tool {
    let input_obj = def
        .input_schema
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    let output_obj = <CliArtifact as McpOutputSchema>::validated_schema()
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);

    let mut tool = Tool::default();
    tool.name = def.name.to_owned().into();
    tool.title = Some(def.title.to_owned());
    tool.description = Some(def.description.to_owned().into());
    tool.input_schema = Arc::new(input_obj);
    tool.output_schema = Some(Arc::new(output_obj));
    tool.annotations = def
        .read_only
        .then(|| ToolAnnotations::new().read_only(true));
    tool.meta = Some(MetaObject(tool_ui_meta(
        SERVER_NAME,
        &default_tool_visibility(),
    )));
    tool
}

#[must_use]
pub fn list_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_SEND,
            title: "Send a Message",
            description: "Send a message to a teammate, one of their agent sessions, or a \
                          channel. `to` is @user for their inbox (never interrupts), \
                          @user/session-handle to reach one running session, or #channel. \
                          Addressing a session that has gone idle falls back to their inbox, \
                          so you never need to check whether someone is online first. Find \
                          handles with comms_sessions.",
            input_schema: schemars::schema_for!(SendInput).to_value(),
            read_only: false,
        }),
        create_tool(&ToolDef {
            name: TOOL_INBOX,
            title: "Read Unread Messages",
            description: "Read messages addressed to you or to this session since you last \
                          looked, newest first, and mark them read. Pass peek=true to read \
                          without advancing the read mark. Each session keeps its own unread \
                          mark, so reading here does not clear another session's inbox.",
            input_schema: schemars::schema_for!(InboxInput).to_value(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_HISTORY,
            title: "Read Conversation History",
            description: "Read past messages with a person (@user) or in a channel \
                          (#channel), newest first, whether or not they were unread.",
            input_schema: schemars::schema_for!(HistoryInput).to_value(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_CHANNELS,
            title: "List Channels",
            description: "List the channels visible to you, with member counts. Channels \
                          deliver to inboxes and never interrupt a running session unless the \
                          channel is marked urgent.",
            input_schema: schemars::schema_for!(ChannelsInput).to_value(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_SESSIONS,
            title: "List Live Sessions",
            description: "The directory of agent sessions running right now: their handle \
                          (the address you send to), whose they are, which repository and \
                          branch they are working in, and what they are doing. Use this to \
                          find the handle before sending to a session.",
            input_schema: schemars::schema_for!(SessionsInput).to_value(),
            read_only: true,
        }),
        create_tool(&ToolDef {
            name: TOOL_WHOAMI,
            title: "Who Am I",
            description: "Who you are on this platform: your account, roles and department, \
                          whether your Odoo login is linked (never the key), exactly which \
                          marketplaces, plugins, MCP servers and skills your role grants — the \
                          same resolver the bridge manifest uses — and your own live sessions. \
                          Returns JSON.",
            input_schema: schemars::schema_for!(WhoamiInput).to_value(),
            read_only: true,
        }),
    ]
}
