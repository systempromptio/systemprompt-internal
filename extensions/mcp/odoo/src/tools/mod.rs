//! Tool definitions exposed by the `odoo` MCP server.
//!
//! Ten tools over four Odoo models: `crm.lead` (search, get, create, update,
//! report), `res.partner` (search, get), `mail.message` (`note_add`) and
//! `mail.activity` (`activity_list`), plus one composite briefing
//! (`business_overview_data`) that exists so a morning summary is one call
//! rather than five.
//!
//! Descriptions say what the tool does *and* whose data it sees, because the
//! answer is not obvious: every call runs as the calling user's own Odoo
//! account, so results are already scoped by Odoo's record rules.

pub mod inputs;

use rmcp::model::{MetaObject, Tool};
use std::sync::Arc;
use systemprompt::mcp::{default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::{CliArtifact, ToolResponse};

use inputs::{
    ActivityListInput, LeadCreateInput, LeadGetInput, LeadReportInput, LeadSearchInput,
    LeadUpdateInput, NoteAddInput, OverviewInput, PartnerGetInput, PartnerSearchInput,
};

pub const SERVER_NAME: &str = "odoo";

pub const TOOL_LEAD_SEARCH: &str = "crm_lead_search";
pub const TOOL_LEAD_GET: &str = "crm_lead_get";
pub const TOOL_LEAD_CREATE: &str = "crm_lead_create";
pub const TOOL_LEAD_UPDATE: &str = "crm_lead_update";
pub const TOOL_LEAD_REPORT: &str = "crm_lead_report";
pub const TOOL_PARTNER_SEARCH: &str = "partner_search";
pub const TOOL_PARTNER_GET: &str = "partner_get";
pub const TOOL_NOTE_ADD: &str = "note_add";
pub const TOOL_ACTIVITY_LIST: &str = "activity_list";
pub const TOOL_OVERVIEW: &str = "business_overview_data";

/// Every tool name this server answers to, for the unknown-tool error.
pub const ALL_TOOLS: [&str; 10] = [
    TOOL_LEAD_SEARCH,
    TOOL_LEAD_GET,
    TOOL_LEAD_CREATE,
    TOOL_LEAD_UPDATE,
    TOOL_LEAD_REPORT,
    TOOL_PARTNER_SEARCH,
    TOOL_PARTNER_GET,
    TOOL_NOTE_ADD,
    TOOL_ACTIVITY_LIST,
    TOOL_OVERVIEW,
];

struct ToolDef<'a> {
    name: &'a str,
    title: &'a str,
    description: &'a str,
    // JSON: protocol boundary
    input_schema: serde_json::Value,
}

fn create_tool(def: &ToolDef<'_>) -> Tool {
    let input_obj = def
        .input_schema
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    let output_obj = ToolResponse::<CliArtifact>::schema()
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
        SERVER_NAME,
        &default_tool_visibility(),
    )));
    tool
}

// Why: the crm.lead tools — the four record operations plus the aggregation.
fn lead_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_LEAD_SEARCH,
            title: "Search CRM Leads",
            description: "Search leads and opportunities in Odoo CRM by free text, stage, or \
                          salesperson. Runs as your own Odoo account, so it returns exactly the \
                          leads Odoo lets you see.",
            input_schema: schemars::schema_for!(LeadSearchInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_GET,
            title: "Get CRM Lead",
            description: "Read one lead or opportunity in full by its Odoo id, including stage, \
                          salesperson, revenue forecast and description.",
            input_schema: schemars::schema_for!(LeadGetInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_CREATE,
            title: "Create CRM Lead",
            description: "Create a lead in Odoo CRM. Only the subject is required. The lead is \
                          created by your Odoo user, so it lands in your pipeline and your name \
                          is on it.",
            input_schema: schemars::schema_for!(LeadCreateInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_UPDATE,
            title: "Update CRM Lead",
            description: "Update fields on an existing lead — including moving it between \
                          pipeline stages with `stage_id`, reassigning with `user_id`, or \
                          re-forecasting `expected_revenue` and `probability`.",
            input_schema: schemars::schema_for!(LeadUpdateInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_REPORT,
            title: "CRM Pipeline Report",
            description: "Aggregate the pipeline by stage or by salesperson: lead count and total \
                          expected revenue per group, optionally limited to a creation-date \
                          window. Use this instead of searching and counting.",
            input_schema: schemars::schema_for!(LeadReportInput).to_value(),
        }),
    ]
}

// Why: everything that is not a lead — partners, chatter, activities, and the
// composite briefing.
fn context_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_PARTNER_SEARCH,
            title: "Search Partners",
            description: "Search Odoo partners (customers, contacts, vendors) by name, email or \
                          phone.",
            input_schema: schemars::schema_for!(PartnerSearchInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_PARTNER_GET,
            title: "Get Partner",
            description: "Read one partner in full by Odoo id: contact details, address, company \
                          and category.",
            input_schema: schemars::schema_for!(PartnerGetInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_NOTE_ADD,
            title: "Log a Note",
            description: "Post a note to a record's chatter in Odoo — a lead, a partner, or any \
                          record with a message thread. The note is attributed to you in Odoo's \
                          audit trail, so write it as yourself.",
            input_schema: schemars::schema_for!(NoteAddInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_ACTIVITY_LIST,
            title: "List My Activities",
            description: "List the scheduled activities assigned to you in Odoo — calls, \
                          meetings, to-dos — optionally only those past their deadline.",
            input_schema: schemars::schema_for!(ActivityListInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_OVERVIEW,
            title: "Business Overview",
            description: "One call for a daily briefing: pipeline by stage, leads created in the \
                          last seven days, your overdue and due-today activities, and the twenty \
                          most recent chatter notes. Prefer this over issuing the individual \
                          queries yourself.",
            input_schema: schemars::schema_for!(OverviewInput).to_value(),
        }),
    ]
}

#[must_use]
pub fn list_tools() -> Vec<Tool> {
    let mut tools = lead_tools();
    tools.extend(context_tools());
    tools
}
