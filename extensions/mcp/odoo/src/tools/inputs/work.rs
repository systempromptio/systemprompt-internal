//! Inputs for the scheduling and collaboration surfaces: activities, calendar
//! events, project tasks, and Discuss channels.
//!
//! Where a field names a person, a project or a channel, it takes the *name* a
//! user would say rather than an Odoo id. Resolving those to ids is the
//! server's job — a caller who has to look up `user_id = 7` first has been
//! handed the database, not a tool.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActivityListInput {
    /// Restrict to activities on one model, e.g. "crm.lead". Omit for all.
    pub model: Option<String>,
    /// Only activities whose deadline has passed.
    pub overdue_only: Option<bool>,
    /// Maximum activities to return (default 20, capped at 100).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActivityCreateInput {
    /// Odoo model the activity hangs off, e.g. "crm.lead".
    pub model: String,
    /// Odoo id of the record.
    pub res_id: i64,
    /// One-line description of what is to be done.
    pub summary: String,
    /// Due date, YYYY-MM-DD.
    pub date_deadline: String,
    /// Who should do it: an Odoo login or display name. Defaults to you.
    pub user: Option<String>,
    /// Longer note shown on the activity.
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActivityCompleteInput {
    /// Odoo id of the activity to mark done.
    pub activity_id: i64,
    /// What happened. Odoo logs this to the record's chatter, so it becomes
    /// part of the permanent history rather than disappearing with the
    /// activity.
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelListInput {
    /// Free text matched against the channel name.
    pub query: Option<String>,
    /// Maximum channels to return (default 20, capped at 100).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelPostInput {
    /// Odoo id of the channel, from `channel_list`.
    pub channel_id: i64,
    /// Message body. Plain text or simple HTML.
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CalendarEventListInput {
    /// Only events starting on or after this date (YYYY-MM-DD).
    pub date_from: Option<String>,
    /// Only events starting on or before this date (YYYY-MM-DD).
    pub date_to: Option<String>,
    /// Free text matched against the event name.
    pub query: Option<String>,
    /// Maximum events to return (default 20, capped at 100).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CalendarEventCreateInput {
    /// Event title.
    pub name: String,
    /// Start, as "YYYY-MM-DD HH:MM:SS" or ISO 8601. Interpreted as UTC, which
    /// is how Odoo stores every datetime.
    pub start: String,
    /// End. Give this or `duration_hours`; omitting both makes it one hour.
    pub stop: Option<String>,
    /// Length in hours, as an alternative to `stop`.
    pub duration_hours: Option<f64>,
    /// Odoo partner ids to invite, from `partner_search`.
    pub attendee_partner_ids: Option<Vec<i64>>,
    pub description: Option<String>,
    /// Model to link the event to, e.g. "crm.lead". Give with `res_id`.
    pub model: Option<String>,
    /// Record id to link the event to. Give with `model`.
    pub res_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskListInput {
    /// Project name to scope to; matched case-insensitively.
    pub project: Option<String>,
    /// Free text matched against the task name.
    pub query: Option<String>,
    /// Exclude tasks in a closed stage. Defaults to true — "what is on my
    /// plate" almost never means the archive.
    pub open_only: Option<bool>,
    /// Maximum tasks to return (default 20, capped at 100).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskCreateInput {
    /// Task title.
    pub name: String,
    /// Project name. Must match an existing project; the error lists the
    /// projects you can see rather than creating one.
    pub project: String,
    pub description: Option<String>,
    /// Assignee: an Odoo login or display name. Defaults to unassigned.
    pub user: Option<String>,
    /// Due date, YYYY-MM-DD.
    pub date_deadline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskUpdateInput {
    /// Odoo id of the task.
    pub id: i64,
    /// Field/value pairs written straight through to Odoo. Use `stage_id` to
    /// move the task, `user_ids` (a list) to reassign, `priority` to flag it.
    // JSON: passed through to Odoo's `write`, so the shape is Odoo's, not
    // ours — a fixed struct here would mean editing this crate every time a
    // deployment adds a custom field.
    pub fields: serde_json::Map<String, serde_json::Value>,
}
