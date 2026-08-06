//! Tool definitions exposed by the `odoo` MCP server.
//!
//! Fifteen tools over five Odoo models: `crm.lead` (search, get, create,
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

pub mod inputs;

use rmcp::model::{MetaObject, Tool};
use std::sync::Arc;
use systemprompt::mcp::{default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::{CliArtifact, ToolResponse};

use inputs::{
    ActivityListInput, AttachmentAddInput, AttachmentGetInput, AttachmentListInput,
    LeadCreateInput, LeadGetInput, LeadReportInput, LeadSearchInput, LeadUpdateInput, NoteAddInput,
    NoteListInput, NoteSearchInput, OverviewInput, PartnerGetInput, PartnerSearchInput,
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
pub const TOOL_NOTE_LIST: &str = "note_list";
pub const TOOL_NOTE_SEARCH: &str = "note_search";
pub const TOOL_ATTACHMENT_ADD: &str = "attachment_add";
pub const TOOL_ATTACHMENT_LIST: &str = "attachment_list";
pub const TOOL_ATTACHMENT_GET: &str = "attachment_get";
pub const TOOL_ACTIVITY_LIST: &str = "activity_list";
pub const TOOL_OVERVIEW: &str = "business_overview_data";

/// Every tool name this server answers to, for the unknown-tool error.
pub const ALL_TOOLS: [&str; 15] = [
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

// Why: the record-anchored knowledge bank — chatter and attachments. These are
// the retrieval tools, and their descriptions carry the routing advice a model
// needs to pick between them and the structured record searches.
fn knowledge_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_NOTE_SEARCH,
            title: "Search Notes",
            description: "Search every note written in Odoo for what is known about a subject, \
                          across leads, partners and any other record type at once. Reach for \
                          this when the question is about knowledge — \"what do we know about \
                          X\", \"what was agreed\", \"has anyone dealt with this before\" — \
                          rather than about a record's fields. Use crm_lead_search instead when \
                          you want leads by stage, owner or revenue. Each hit names the record \
                          it is attached to, so you can follow it with crm_lead_get, \
                          partner_get or note_list.",
            input_schema: schemars::schema_for!(NoteSearchInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_NOTE_LIST,
            title: "Read a Record's Chatter",
            description: "Read the full note history on one record, newest first, once you know \
                          which record you care about. This is the follow-up to note_search or \
                          crm_lead_search — it gives the whole conversation rather than a \
                          snippet.",
            input_schema: schemars::schema_for!(NoteListInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_NOTE_ADD,
            title: "Log a Note",
            description: "Post a note to a record's chatter in Odoo — a lead, a partner, or any \
                          record with a message thread. This is how knowledge gets *into* the \
                          bank, so write what a colleague would need later, not a restatement \
                          of the record's fields. The note is attributed to you in Odoo's audit \
                          trail, so write it as yourself.",
            input_schema: schemars::schema_for!(NoteAddInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_ATTACHMENT_LIST,
            title: "List Attachments",
            description: "List files attached to Odoo records, optionally scoped to one model, \
                          one record, or a filename fragment. Attachment ids are global; res_id \
                          is only meaningful alongside model.",
            input_schema: schemars::schema_for!(AttachmentListInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_ATTACHMENT_GET,
            title: "Get Attachment",
            description: "Read one attachment's metadata, plus its base64 content when the file \
                          is 1 MB or smaller. Larger files return metadata and a pointer to the \
                          Odoo web UI, because the content would not usefully fit in context.",
            input_schema: schemars::schema_for!(AttachmentGetInput).to_value(),
        }),
        create_tool(&ToolDef {
            name: TOOL_ATTACHMENT_ADD,
            title: "Attach a File",
            description: "Attach a base64-encoded file to an Odoo record, up to 5 MB decoded. \
                          The upload is made by your Odoo user, so your name is on it.",
            input_schema: schemars::schema_for!(AttachmentAddInput).to_value(),
        }),
    ]
}

// Why: everything that is not a lead or a knowledge record — partners,
// activities, and the composite briefing.
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
    tools.extend(knowledge_tools());
    tools.extend(context_tools());
    tools
}
