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
    pub model: Option<String>,
    pub overdue_only: Option<bool>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActivityCreateInput {
    pub model: String,
    pub res_id: i64,
    pub summary: String,
    pub date_deadline: String,
    pub user: Option<String>,
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ActivityCompleteInput {
    pub activity_id: i64,
    pub feedback: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelListInput {
    pub query: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ChannelPostInput {
    pub channel_id: i64,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CalendarEventListInput {
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub query: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CalendarEventCreateInput {
    pub name: String,
    pub start: String,
    pub stop: Option<String>,
    pub duration_hours: Option<f64>,
    pub attendee_partner_ids: Option<Vec<i64>>,
    pub description: Option<String>,
    pub model: Option<String>,
    pub res_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskListInput {
    pub project: Option<String>,
    pub query: Option<String>,
    pub open_only: Option<bool>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskCreateInput {
    pub name: String,
    pub project: String,
    pub description: Option<String>,
    pub user: Option<String>,
    pub date_deadline: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct TaskUpdateInput {
    pub id: i64,
    /// Field/value pairs written straight through to Odoo. Use `stage_id` to
    /// move the task, `user_ids` (a list) to reassign, `priority` to flag it.
    // JSON: passed through to Odoo's `write`, so the shape is Odoo's, not
    // ours — a fixed struct here would mean editing this crate every time a
    // deployment adds a custom field.
    pub fields: serde_json::Map<String, serde_json::Value>,
}
