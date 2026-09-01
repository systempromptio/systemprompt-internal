//! Input shapes for the odoo tools.
//!
//! Field names match Odoo's own (`partner_name`, `email_from`, `stage_id`) so
//! that a model reading Odoo documentation, or an error message quoting a
//! field, does not have to translate.
//!
//! Split by plane: the CRM record inputs live here, the record-anchored
//! knowledge bank in [`knowledge`], and the scheduling and collaboration
//! surfaces in [`work`].

pub mod knowledge;
pub mod work;

pub use knowledge::{
    AttachmentAddInput, AttachmentGetInput, AttachmentListInput, NoteAddInput, NoteListInput,
    NoteSearchInput,
};
pub use work::{
    ActivityCompleteInput, ActivityCreateInput, ActivityListInput, CalendarEventCreateInput,
    CalendarEventListInput, ChannelListInput, ChannelPostInput, TaskCreateInput, TaskListInput,
    TaskUpdateInput,
};

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

pub const DEFAULT_LIMIT: u32 = 20;
pub const MAX_LIMIT: u32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadSearchInput {
    pub query: Option<String>,
    pub stage: Option<String>,
    pub user: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct LeadGetInput {
    pub id: i64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct LeadDeleteInput {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadCreateInput {
    pub name: String,
    pub partner_name: Option<String>,
    pub email_from: Option<String>,
    pub phone: Option<String>,
    pub description: Option<String>,
    pub expected_revenue: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadUpdateInput {
    pub id: i64,
    /// Field/value pairs written straight through to Odoo. Use `stage_id`
    /// (integer) to move a lead between pipeline stages, `user_id` to
    /// reassign, `expected_revenue` / `probability` to re-forecast.
    // JSON: passed through to Odoo's `write`, so the shape is Odoo's, not
    // ours — a fixed struct here would mean editing this crate every time a
    // deployment adds a custom field.
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ReportGroupBy {
    Stage,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadReportInput {
    pub group_by: ReportGroupBy,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PartnerSearchInput {
    pub query: String,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct PartnerGetInput {
    pub id: i64,
}

/// `business_overview_data` takes no arguments.
///
/// The briefing is defined by who is asking, not by what they ask for. Braces
/// rather than a unit struct: every MCP client sends `{}` for "no arguments",
/// and serde refuses to read a map into a unit.
#[expect(
    clippy::empty_structs_with_brackets,
    reason = "serde must accept the `{}` every MCP client sends; a unit struct rejects it"
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct OverviewInput {}

#[must_use]
pub fn resolve_limit(requested: Option<u32>) -> u32 {
    requested.map_or(DEFAULT_LIMIT, |l| l.clamp(1, MAX_LIMIT))
}
