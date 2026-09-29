//! Tool definitions exposed by the `systemprompt` MCP server.

use rmcp::model::{MetaObject, Tool};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use systemprompt::mcp::{
    McpOutputSchema, McpToolHandler, WEBSITE_URL, default_tool_visibility, tool_ui_meta,
};
use systemprompt::models::artifacts::CliArtifact;

pub const SERVER_NAME: &str = "systemprompt";

// Why: every tool name is a `pub const TOOL_*` so
// scripts/check-mcp-tool-names.sh can build its catalog from source; an inline
// literal is invisible to it.
pub const TOOL_SYSTEMPROMPT: &str = "systemprompt";
pub const TOOL_ADMIN_REPORT: &str = "admin_report";
pub const TOOL_USER_ACTIVITY: &str = "user_activity";
pub const TOOL_CONVERSATION_LIST: &str = "conversation_list";
pub const TOOL_USAGE_BY_USER: &str = "usage_by_user";
pub const TOOL_REQUEST_LOG: &str = "request_log";
pub const TOOL_CONVERSATION_AUDIT: &str = "conversation_audit";
pub const TOOL_USERS: &str = "users";

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CliInput {
    #[schemars(
        description = "The CLI command to execute (without 'systemprompt' prefix). Examples: 'plugins run discord send \"message\"', 'core skills list'"
    )]
    pub command: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

#[must_use]
// JSON: protocol boundary
pub fn input_schema() -> serde_json::Value {
    schemars::schema_for!(CliInput).to_value()
}

#[must_use]
// JSON: protocol boundary
pub fn output_schema() -> serde_json::Value {
    <CliArtifact as McpOutputSchema>::validated_schema()
}

struct ToolDef<'a> {
    server_name: &'a str,
    name: &'a str,
    title: &'a str,
    description: &'a str,
    // JSON: protocol boundary
    input_schema: &'a serde_json::Value,
    // JSON: protocol boundary
    output_schema: &'a serde_json::Value,
}

fn create_tool(def: &ToolDef<'_>) -> Tool {
    let input_obj = def
        .input_schema
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    let output_obj = def
        .output_schema
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);

    let mut tool = Tool::default();
    tool.name = def.name.to_owned().into();
    tool.title = Some(def.title.to_owned());
    tool.description = Some(def.description.to_owned().into());
    tool.input_schema = Arc::new(input_obj);
    tool.output_schema = Some(Arc::new(output_obj));
    tool.meta = Some(MetaObject(tool_ui_meta(
        def.server_name,
        &default_tool_visibility(),
    )));
    tool
}

#[must_use]
pub fn list_tools() -> Vec<Tool> {
    let desc = format!(
        "Execute SystemPrompt CLI commands. Pass the command WITHOUT the 'systemprompt' prefix.\n\n\
        For people's activity, conversations, spend, request logs, audits and the user roster use \
        the typed tools instead — user_activity, conversation_list, usage_by_user, request_log, \
        conversation_audit, users — they carry their \
        flags in the schema, page, and never return more than a model can read.\n\n\
        Common commands:\n  \
        - core skills list: List installed skills\n  \
        - core skills show <id>: Show a skill's config and instruction body\n  \
        - core content list: List markdown content\n  \
        - analytics costs summary --since 7d: Spend totals\n  \
        - infra logs request list --since 7d --user <id> --limit 50: Requests (also --until, \
        --model, --provider, --before <cursor>)\n  \
        - infra logs audit <request-id> --messages --limit 20 --max-content 400: One request's \
        transcript, paged\n  \
        - admin users role promote <id>: Grant admin\n\n\
        Never pass --json, --format or --export (they are stripped). On an unknown-flag error \
        run '<command> --help' once; do not guess flags.\n\n\
        Example: {{\"command\": \"core skills list\"}}\n\n\
        Full documentation: {WEBSITE_URL}/docs"
    );
    let mut tools = vec![create_tool(&ToolDef {
        server_name: SERVER_NAME,
        name: TOOL_SYSTEMPROMPT,
        title: "SystemPrompt CLI",
        description: &desc,
        input_schema: &input_schema(),
        output_schema: &output_schema(),
    })];
    let location = crate::CliLocation {
        bin: std::path::PathBuf::default(),
        workdir: std::path::PathBuf::default(),
    };
    let cli = &location;
    let token = "";
    tools.push(crate::reports::ReportHandler { cli, token }.tool_definition(SERVER_NAME));
    tools.push(crate::typed::UserActivityHandler { cli, token }.tool_definition(SERVER_NAME));
    tools.push(crate::typed::ConversationListHandler { cli, token }.tool_definition(SERVER_NAME));
    tools.push(crate::typed::UsageByUserHandler { cli, token }.tool_definition(SERVER_NAME));
    tools.push(crate::typed::RequestLogHandler { cli, token }.tool_definition(SERVER_NAME));
    tools.push(crate::typed::ConversationAuditHandler { cli, token }.tool_definition(SERVER_NAME));
    tools.push(crate::typed::UsersHandler { cli, token }.tool_definition(SERVER_NAME));
    tools
}
