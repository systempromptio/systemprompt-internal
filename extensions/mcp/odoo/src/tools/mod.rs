//! Tool definitions exposed by the `odoo` MCP server.
//!
//! Thirty-eight tools over the models a business actually runs on: `crm.lead`
//! (search, get, create, update, delete, report, and the closing actions —
//! won, lost, convert), `res.partner` (search, get, create, update),
//! `mail.message` (`note_add`, `note_list`, `note_search`), `ir.attachment`,
//! `mail.activity`, `sale.order` and `account.move`, plus one composite
//! briefing (`business_overview_data`) that exists so a morning summary is one
//! call rather than five.
//!
//! The discovery tools — `crm_stage_list`, `user_list`, `activity_type_list` —
//! exist because the alternative is a model guessing integer ids. A stage move
//! used to require knowing `stage_id` numerically; nothing listed the stages,
//! so the guess was the only option and a wrong one moved the wrong deal.
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
pub const TOOL_LEAD_DELETE: &str = "crm_lead_delete";
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
pub const TOOL_PARTNER_CREATE: &str = "partner_create";
pub const TOOL_PARTNER_UPDATE: &str = "partner_update";
pub const TOOL_LEAD_MARK_WON: &str = "crm_lead_mark_won";
pub const TOOL_LEAD_MARK_LOST: &str = "crm_lead_mark_lost";
pub const TOOL_LEAD_CONVERT: &str = "crm_lead_convert_to_opportunity";
pub const TOOL_STAGE_LIST: &str = "crm_stage_list";
pub const TOOL_USER_LIST: &str = "user_list";
pub const TOOL_ACTIVITY_TYPE_LIST: &str = "activity_type_list";
pub const TOOL_SALE_ORDER_LIST: &str = "sale_order_list";
pub const TOOL_SALE_ORDER_GET: &str = "sale_order_get";
pub const TOOL_SALE_ORDER_CREATE: &str = "sale_order_create";
pub const TOOL_INVOICE_LIST: &str = "invoice_list";
pub const TOOL_INVOICE_GET: &str = "invoice_get";

pub const ALL_TOOLS: [&str; 38] = [
    TOOL_LEAD_SEARCH,
    TOOL_LEAD_GET,
    TOOL_LEAD_CREATE,
    TOOL_LEAD_UPDATE,
    TOOL_LEAD_DELETE,
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
    TOOL_PARTNER_CREATE,
    TOOL_PARTNER_UPDATE,
    TOOL_LEAD_MARK_WON,
    TOOL_LEAD_MARK_LOST,
    TOOL_LEAD_CONVERT,
    TOOL_STAGE_LIST,
    TOOL_USER_LIST,
    TOOL_ACTIVITY_TYPE_LIST,
    TOOL_SALE_ORDER_LIST,
    TOOL_SALE_ORDER_GET,
    TOOL_SALE_ORDER_CREATE,
    TOOL_INVOICE_LIST,
    TOOL_INVOICE_GET,
];

/// What a tool does to Odoo, as advertised through `ToolAnnotations`.
///
/// Advisory only: the governance chain is the enforced gate; this is the hint
/// a well-behaved client uses to decide whether to ask the user before calling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Effect {
    ReadOnly,
    Write,
    Destructive,
}

pub(crate) struct ToolDef<'a> {
    pub name: &'a str,
    pub title: &'a str,
    pub description: &'a str,
    // JSON: protocol boundary
    pub input_schema: serde_json::Value,
    pub effect: Effect,
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
    tool.annotations = match def.effect {
        Effect::ReadOnly => Some(ToolAnnotations::new().read_only(true)),
        Effect::Write => None,
        // Why: a client honouring destructiveHint prompts before calling. The
        // governance stage is the enforced gate; this is the advisory one.
        Effect::Destructive => Some(ToolAnnotations::new().destructive(true)),
    };
    tool.meta = Some(MetaObject(tool_ui_meta(
        SERVER_NAME,
        &default_tool_visibility(),
    )));
    tool
}

#[must_use]
pub fn list_tools() -> Vec<Tool> {
    let mut tools = catalog::lead_tools();
    tools.extend(catalog::closing_tools());
    tools.extend(catalog::discovery_tools());
    tools.extend(catalog::partner_write_tools());
    tools.extend(catalog::sales_tools());
    tools.extend(catalog::knowledge_tools());
    tools.extend(catalog::work_tools());
    tools.extend(catalog::context_tools());
    tools
}
