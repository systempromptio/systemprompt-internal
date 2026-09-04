//! The knowledge and work planes: chatter, attachments, activities, calendar,
//! tasks and channels.
//!
//! Grouped away from the record tools in [`super`] because they describe what
//! people wrote and what they owe, not what the CRM holds.

use rmcp::model::Tool;

use crate::tools::inputs::{
    ActivityCompleteInput, ActivityCreateInput, AttachmentAddInput, AttachmentGetInput,
    AttachmentListInput, CalendarEventCreateInput, CalendarEventListInput, ChannelListInput,
    ChannelPostInput, NoteAddInput, NoteListInput, NoteSearchInput, TaskCreateInput, TaskListInput,
    TaskUpdateInput,
};
use crate::tools::{
    Effect, TOOL_ACTIVITY_COMPLETE, TOOL_ACTIVITY_CREATE, TOOL_ATTACHMENT_ADD, TOOL_ATTACHMENT_GET,
    TOOL_ATTACHMENT_LIST, TOOL_CALENDAR_EVENT_CREATE, TOOL_CALENDAR_EVENT_LIST, TOOL_CHANNEL_LIST,
    TOOL_CHANNEL_POST, TOOL_NOTE_ADD, TOOL_NOTE_LIST, TOOL_NOTE_SEARCH, TOOL_TASK_CREATE,
    TOOL_TASK_LIST, TOOL_TASK_UPDATE, ToolDef, create_tool,
};

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
