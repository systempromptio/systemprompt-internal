//! Tool definitions exposed by the `odoo` MCP server.
//!
//! Twenty-four tools over ten Odoo models: `crm.lead` (search, get, create,
//! update, report), `res.partner` (search, get), `mail.message` (`note_add`,
//! `note_list`, `note_search`), `ir.attachment` (`attachment_add`,
//! `attachment_list`, `attachment_get`) and `mail.activity` (`activity_list`),
//! plus one composite briefing (`business_overview_data`) that exists so a
//! morning summary is one call rather than five.
//!
//! Odoo Community ships no Knowledge app, so the knowledge bank is the chatter
//! and attachments already anchored to business records. `note_search` is the
//! way into it and the descriptions below say so explicitly: a model choosing
//! between tools needs to know that `crm_lead_search` finds records by their
//! fields while `note_search` finds what people wrote.
//!
//! Descriptions say what the tool does *and* whose data it sees, because the
//! answer is not obvious: every call runs as the calling user's own Odoo
//! account, so results are already scoped by Odoo's record rules.

pub mod catalog;
pub mod inputs;

use rmcp::model::{MetaObject, Tool, ToolAnnotations};
use std::sync::Arc;
use systemprompt::mcp::{McpOutputSchema, default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::CliArtifact;

pub const SERVER_NAME: &str = "odoo";

pub const TOOL_LEAD_SEARCH: &str = "crm_lead_search";
pub const TOOL_LEAD_GET: &str = "crm_lead_get";
pub const TOOL_LEAD_CREATE: &str = "crm_lead_create";
pub const TOOL_LEAD_UPDATE: &str = "crm_lead_update";
pub const TOOL_LEAD_REPORT: &str = "crm_lead_report";
pub const TOOL_PARTNER_SEARCH: &str = "partner_search";
pub const TOOL_PARTNER_GET: &str = "partner_get";
pub const TOOL_NOTE_ADD: &str = "note_add";
pub const TOOL_NOTE_LIST: &str = "note_list";
pub const TOOL_NOTE_SEARCH: &str = "note_search";
pub const TOOL_ATTACHMENT_ADD: &str = "attachment_add";
pub const TOOL_ATTACHMENT_LIST: &str = "attachment_list";
pub const TOOL_ATTACHMENT_GET: &str = "attachment_get";
pub const TOOL_ACTIVITY_LIST: &str = "activity_list";
pub const TOOL_ACTIVITY_CREATE: &str = "activity_create";
pub const TOOL_ACTIVITY_COMPLETE: &str = "activity_complete";
pub const TOOL_CHANNEL_LIST: &str = "channel_list";
pub const TOOL_CHANNEL_POST: &str = "channel_post";
pub const TOOL_CALENDAR_EVENT_LIST: &str = "calendar_event_list";
pub const TOOL_CALENDAR_EVENT_CREATE: &str = "calendar_event_create";
pub const TOOL_TASK_LIST: &str = "task_list";
pub const TOOL_TASK_CREATE: &str = "task_create";
pub const TOOL_TASK_UPDATE: &str = "task_update";
pub const TOOL_OVERVIEW: &str = "business_overview_data";

pub const ALL_TOOLS: [&str; 24] = [
    TOOL_LEAD_SEARCH,
    TOOL_LEAD_GET,
    TOOL_LEAD_CREATE,
    TOOL_LEAD_UPDATE,
    TOOL_LEAD_REPORT,
    TOOL_PARTNER_SEARCH,
    TOOL_PARTNER_GET,
    TOOL_NOTE_ADD,
    TOOL_NOTE_LIST,
    TOOL_NOTE_SEARCH,
    TOOL_ATTACHMENT_ADD,
    TOOL_ATTACHMENT_LIST,
    TOOL_ATTACHMENT_GET,
    TOOL_ACTIVITY_LIST,
    TOOL_ACTIVITY_CREATE,
    TOOL_ACTIVITY_COMPLETE,
    TOOL_CHANNEL_LIST,
    TOOL_CHANNEL_POST,
    TOOL_CALENDAR_EVENT_LIST,
    TOOL_CALENDAR_EVENT_CREATE,
    TOOL_TASK_LIST,
    TOOL_TASK_CREATE,
    TOOL_TASK_UPDATE,
    TOOL_OVERVIEW,
];

pub(crate) struct ToolDef<'a> {
    pub name: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    // JSON: protocol boundary
    pub input_schema: serde_json::Value,
    pub read_only: bool,
}

pub(crate) fn create_tool(def: &ToolDef<'_>) -> Tool {
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
    let mut tools = catalog::lead_tools();
    tools.extend(catalog::knowledge_tools());
    tools.extend(catalog::work_tools());
    tools.extend(catalog::context_tools());
    tools
}
