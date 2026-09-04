//! The tool definitions, grouped by plane.
//!
//! Split from [`super`] only for size: each group is a builder returning its
//! slice of [`super::list_tools`]. Descriptions are written for an agent
//! choosing between tools, not for a human reading a manual — where two tools
//! could plausibly answer the same question, the description says which to
//! reach for.

use rmcp::model::Tool;

use super::inputs::{
    ActivityCompleteInput, ActivityCreateInput, ActivityListInput, ActivityTypeListInput,
    AttachmentAddInput, AttachmentGetInput, AttachmentListInput, CalendarEventCreateInput,
    CalendarEventListInput, ChannelListInput, ChannelPostInput, InvoiceGetInput, InvoiceListInput,
    LeadConvertInput, LeadCreateInput, LeadDeleteInput, LeadGetInput, LeadMarkLostInput,
    LeadMarkWonInput, LeadReportInput, LeadSearchInput, LeadUpdateInput, NoteAddInput,
    NoteListInput, NoteSearchInput, OverviewInput, PartnerCreateInput, PartnerGetInput,
    PartnerSearchInput, PartnerUpdateInput, SaleOrderCreateInput, SaleOrderGetInput,
    SaleOrderListInput, StageListInput, TaskCreateInput, TaskListInput, TaskUpdateInput,
    UserListInput,
};
use super::{
    Effect, TOOL_ACTIVITY_COMPLETE, TOOL_ACTIVITY_CREATE, TOOL_ACTIVITY_LIST,
    TOOL_ACTIVITY_TYPE_LIST, TOOL_ATTACHMENT_ADD, TOOL_ATTACHMENT_GET, TOOL_ATTACHMENT_LIST,
    TOOL_CALENDAR_EVENT_CREATE, TOOL_CALENDAR_EVENT_LIST, TOOL_CHANNEL_LIST, TOOL_CHANNEL_POST,
    TOOL_INVOICE_GET, TOOL_INVOICE_LIST, TOOL_LEAD_CONVERT, TOOL_LEAD_CREATE, TOOL_LEAD_DELETE,
    TOOL_LEAD_GET, TOOL_LEAD_MARK_LOST, TOOL_LEAD_MARK_WON, TOOL_LEAD_REPORT, TOOL_LEAD_SEARCH,
    TOOL_LEAD_UPDATE, TOOL_NOTE_ADD, TOOL_NOTE_LIST, TOOL_NOTE_SEARCH, TOOL_OVERVIEW,
    TOOL_PARTNER_CREATE, TOOL_PARTNER_GET, TOOL_PARTNER_SEARCH, TOOL_PARTNER_UPDATE,
    TOOL_SALE_ORDER_CREATE, TOOL_SALE_ORDER_GET, TOOL_SALE_ORDER_LIST, TOOL_STAGE_LIST,
    TOOL_TASK_CREATE, TOOL_TASK_LIST, TOOL_TASK_UPDATE, TOOL_USER_LIST, ToolDef, create_tool,
};

pub fn lead_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_LEAD_SEARCH,
            title: "Search CRM Leads",
            description: "Search leads and opportunities in Odoo CRM by free text, stage, or \
                          salesperson. Runs as your own Odoo account, so it returns exactly the \
                          leads Odoo lets you see.",
            input_schema: schemars::schema_for!(LeadSearchInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_GET,
            title: "Get CRM Lead",
            description: "Read one lead or opportunity in full by its Odoo id, including stage, \
                          salesperson, revenue forecast and description.",
            input_schema: schemars::schema_for!(LeadGetInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_CREATE,
            title: "Create CRM Lead",
            description: "Create a lead in Odoo CRM. Only the subject is required. The lead is \
                          created by your Odoo user, so it lands in your pipeline and your name \
                          is on it.",
            input_schema: schemars::schema_for!(LeadCreateInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_UPDATE,
            title: "Update CRM Lead",
            description: "Update fields on an existing lead — move it between pipeline stages \
                          by passing `stage` as a NAME (call crm_stage_list first), reassign \
                          with `user_id`, or re-forecast `expected_revenue` and `probability` \
                          through `fields`. To close a deal use crm_lead_mark_won or \
                          crm_lead_mark_lost instead: they run Odoo's own closing actions \
                          rather than writing a probability by hand.",
            input_schema: schemars::schema_for!(LeadUpdateInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_DELETE,
            title: "Delete CRM Lead",
            description: "Permanently delete a lead from Odoo CRM by id. This is `unlink`, not \
                          archive — the record, its chatter and its activities are gone and \
                          cannot be recovered. Use crm_lead_update with `{\"active\": false}` \
                          to archive instead. Runs as your own Odoo account, so Odoo refuses it \
                          unless you may delete leads.",
            input_schema: schemars::schema_for!(LeadDeleteInput).to_value(),
            effect: Effect::Destructive,
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_REPORT,
            title: "CRM Pipeline Report",
            description: "Aggregate the pipeline by stage or by salesperson: lead count and total \
                          expected revenue per group, optionally limited to a creation-date \
                          window. Use this instead of searching and counting.",
            input_schema: schemars::schema_for!(LeadReportInput).to_value(),
            effect: Effect::ReadOnly,
        }),
    ]
}

/// The closing actions — the reason a pipeline can be emptied as well as
/// filled. Split from [`lead_tools`] to keep each builder readable.
pub fn closing_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_LEAD_MARK_WON,
            title: "Mark Lead Won",
            description: "Close a deal as WON. Runs Odoo's own win action, so the stage, the \
                          probability and the close date all move together and the pipeline \
                          report agrees with the dashboard. Prefer this over writing \
                          `probability` by hand.",
            input_schema: schemars::schema_for!(LeadMarkWonInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_MARK_LOST,
            title: "Mark Lead Lost",
            description: "Close a deal as LOST, optionally recording why in the words the team \
                          would use. Runs Odoo's own lost action; the lead is closed, not \
                          deleted, so it still counts in the pipeline history.",
            input_schema: schemars::schema_for!(LeadMarkLostInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_LEAD_CONVERT,
            title: "Convert Lead to Opportunity",
            description: "Promote a qualified lead into an opportunity, optionally against an \
                          existing customer. Use it when a raw enquiry has become a deal worth \
                          forecasting.",
            input_schema: schemars::schema_for!(LeadConvertInput).to_value(),
            effect: Effect::Write,
        }),
    ]
}

/// The lookups that turn a name a person said into an id Odoo needs.
///
/// These exist because the alternative is a model guessing. Every one of them
/// is cheap, and a wrong guess writes to the wrong record.
pub fn discovery_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_STAGE_LIST,
            title: "List Pipeline Stages",
            description: "List the CRM pipeline's stages in order, with their ids and which one \
                          counts as won. Call this before moving a lead: stage ids are \
                          deployment-specific and cannot be guessed.",
            input_schema: schemars::schema_for!(StageListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_USER_LIST,
            title: "List Salespeople",
            description: "List the Odoo users a lead or task can be assigned to, with their ids. \
                          Use it to resolve a name before reassigning work.",
            input_schema: schemars::schema_for!(UserListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_ACTIVITY_TYPE_LIST,
            title: "List Activity Types",
            description: "List the activity types this Odoo defines — typically Call, Meeting, \
                          Email and To Do. Pass one by name to activity_create to schedule a \
                          call rather than a generic to-do.",
            input_schema: schemars::schema_for!(ActivityTypeListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
    ]
}

/// `res.partner` writes: the customer database itself.
pub fn partner_write_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_PARTNER_CREATE,
            title: "Create Customer",
            description: "Create a customer or contact in Odoo. Search first with partner_search \
                          — a duplicate customer splits a company's history in two. Pass the new \
                          id to crm_lead_create so the lead joins the contact database.",
            input_schema: schemars::schema_for!(PartnerCreateInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_PARTNER_UPDATE,
            title: "Update Customer",
            description: "Update fields on an existing customer or contact — email, phone, \
                          address.",
            input_schema: schemars::schema_for!(PartnerUpdateInput).to_value(),
            effect: Effect::Write,
        }),
    ]
}

/// Quote-to-cash: `sale.order` (writable) and `account.move` (read-only).
pub fn sales_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_SALE_ORDER_LIST,
            title: "List Quotations and Orders",
            description: "List sales orders and quotations, optionally for one customer, one \
                          state (`draft`/`sent` are quotations, `sale` is confirmed) or a date \
                          window. Runs as your own Odoo account.",
            input_schema: schemars::schema_for!(SaleOrderListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_SALE_ORDER_GET,
            title: "Get Quotation or Order",
            description: "Read one sales order in full by id, including every order line with \
                          its product, quantity and price.",
            input_schema: schemars::schema_for!(SaleOrderGetInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_SALE_ORDER_CREATE,
            title: "Raise a Quotation",
            description: "Create a quotation for a customer from a list of product lines. It is \
                          created as a draft, never confirmed — a human sends it. Set `origin` \
                          to the lead it came from so the deal and the quote read together.",
            input_schema: schemars::schema_for!(SaleOrderCreateInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_INVOICE_LIST,
            title: "List Customer Invoices",
            description: "List customer invoices with what is still outstanding on each. Set \
                          `unpaid_only` to answer 'who owes us'. Read-only: this server never \
                          raises or posts an invoice.",
            input_schema: schemars::schema_for!(InvoiceListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_INVOICE_GET,
            title: "Get Customer Invoice",
            description: "Read one customer invoice in full by id, including its lines and the \
                          balance still due.",
            input_schema: schemars::schema_for!(InvoiceGetInput).to_value(),
            effect: Effect::ReadOnly,
        }),
    ]
}

pub fn knowledge_tools() -> Vec<Tool> {
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
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_NOTE_LIST,
            title: "Read a Record's Chatter",
            description: "Read the full note history on one record, newest first, once you know \
                          which record you care about. This is the follow-up to note_search or \
                          crm_lead_search — it gives the whole conversation rather than a \
                          snippet.",
            input_schema: schemars::schema_for!(NoteListInput).to_value(),
            effect: Effect::ReadOnly,
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
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_ATTACHMENT_LIST,
            title: "List Attachments",
            description: "List files attached to Odoo records, optionally scoped to one model, \
                          one record, or a filename fragment. Each row says whether it is a \
                          stored file or a link to one held elsewhere. Attachment ids are \
                          global; res_id is only meaningful alongside model.",
            input_schema: schemars::schema_for!(AttachmentListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_ATTACHMENT_GET,
            title: "Get Attachment",
            description: "Read one attachment's metadata, plus its base64 content when it is a \
                          stored file of 1 MB or less. A link-type attachment returns its URL \
                          and no content — there are no bytes in Odoo to return. Larger stored \
                          files return metadata and a pointer to the Odoo web UI, because the \
                          content would not usefully fit in context.",
            input_schema: schemars::schema_for!(AttachmentGetInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_ATTACHMENT_ADD,
            title: "Attach a File",
            description: "Attach a file to an Odoo record, either by uploading it \
                          (content_base64, up to 5 MB decoded) or by recording a link to it \
                          (url) — use the link form for large external media such as meeting \
                          recordings in object storage. Provide exactly one of the two. The \
                          attachment is created by your Odoo user, so your name is on it.",
            input_schema: schemars::schema_for!(AttachmentAddInput).to_value(),
            effect: Effect::Write,
        }),
    ]
}

pub fn work_tools() -> Vec<Tool> {
    let mut tools = scheduling_tools();
    tools.extend(collaboration_tools());
    tools
}

fn scheduling_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_ACTIVITY_CREATE,
            title: "Schedule an Activity",
            description: "Schedule a follow-up on an Odoo record — a call, a to-do, a reminder — \
                          due on a date and assigned to someone. Defaults to you if no user is \
                          named. Use this for work attached to a lead or partner; use \
                          calendar_event_create for something with a time and attendees.",
            input_schema: schemars::schema_for!(ActivityCreateInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_ACTIVITY_COMPLETE,
            title: "Complete an Activity",
            description: "Mark an activity done and log what happened. The feedback is written \
                          to the record's chatter, so it survives as history rather than \
                          disappearing with the reminder — write it as a note to whoever reads \
                          the record next.",
            input_schema: schemars::schema_for!(ActivityCompleteInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_CALENDAR_EVENT_LIST,
            title: "List Calendar Events",
            description: "List calendar events in a date window, optionally filtered by title. \
                          Use this for what is on the calendar; use activity_list for to-dos \
                          that have a deadline but no meeting time.",
            input_schema: schemars::schema_for!(CalendarEventListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_CALENDAR_EVENT_CREATE,
            title: "Create a Calendar Event",
            description: "Create a calendar event. Give a start and either a stop or \
                          duration_hours (one hour if neither). Datetimes are UTC. Invite people \
                          with partner ids from partner_search, and link the event to a lead by \
                          passing model and res_id together.",
            input_schema: schemars::schema_for!(CalendarEventCreateInput).to_value(),
            effect: Effect::Write,
        }),
    ]
}

fn collaboration_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_TASK_LIST,
            title: "List Tasks",
            description: "List project tasks — open ones unless you ask otherwise — optionally \
                          scoped to a project by name or filtered by title.",
            input_schema: schemars::schema_for!(TaskListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_TASK_CREATE,
            title: "Create a Task",
            description: "Create a task in an existing project, named rather than by id. The \
                          project must already exist: if the name does not match, the error \
                          lists the projects you can see rather than creating a new one.",
            input_schema: schemars::schema_for!(TaskCreateInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_TASK_UPDATE,
            title: "Update a Task",
            description: "Update fields on a task — `stage_id` to move it along, `user_ids` (a \
                          list) to reassign, `date_deadline`, `priority`.",
            input_schema: schemars::schema_for!(TaskUpdateInput).to_value(),
            effect: Effect::Write,
        }),
        create_tool(&ToolDef {
            name: TOOL_CHANNEL_LIST,
            title: "List Discuss Channels",
            description: "List the Odoo Discuss channels you can see, with their type and member \
                          count. Run this first to get the channel id channel_post needs.",
            input_schema: schemars::schema_for!(ChannelListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_CHANNEL_POST,
            title: "Post to a Channel",
            description: "Post a message to a Discuss channel as yourself. This talks to a room \
                          of people and leaves no trace on any record — use note_add instead \
                          when the point is to document something against a lead or partner.",
            input_schema: schemars::schema_for!(ChannelPostInput).to_value(),
            effect: Effect::Write,
        }),
    ]
}

pub fn context_tools() -> Vec<Tool> {
    vec![
        create_tool(&ToolDef {
            name: TOOL_PARTNER_SEARCH,
            title: "Search Partners",
            description: "Search Odoo partners (customers, contacts, vendors) by name, email or \
                          phone.",
            input_schema: schemars::schema_for!(PartnerSearchInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_PARTNER_GET,
            title: "Get Partner",
            description: "Read one partner in full by Odoo id: contact details, address, company \
                          and category.",
            input_schema: schemars::schema_for!(PartnerGetInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_ACTIVITY_LIST,
            title: "List My Activities",
            description: "List the scheduled activities assigned to you in Odoo — calls, \
                          meetings, to-dos — optionally only those past their deadline.",
            input_schema: schemars::schema_for!(ActivityListInput).to_value(),
            effect: Effect::ReadOnly,
        }),
        create_tool(&ToolDef {
            name: TOOL_OVERVIEW,
            title: "Business Overview",
            description: "One call for a daily briefing: pipeline by stage, leads created in the \
                          last seven days, your overdue and due-today activities, and the twenty \
                          most recent chatter notes. Prefer this over issuing the individual \
                          queries yourself.",
            input_schema: schemars::schema_for!(OverviewInput).to_value(),
            effect: Effect::ReadOnly,
        }),
    ]
}
