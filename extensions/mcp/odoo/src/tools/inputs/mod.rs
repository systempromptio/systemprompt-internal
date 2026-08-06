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

/// Records returned when a tool's `limit` is omitted.
pub const DEFAULT_LIMIT: u32 = 20;
/// Ceiling on `limit`, applied silently. A model asking for 5000 leads wants
/// "all of them"; giving it 100 is more useful than a context overflow.
pub const MAX_LIMIT: u32 = 100;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadSearchInput {
    /// Free text matched against the lead subject, contact name and email.
    pub query: Option<String>,
    /// Stage name to filter on, e.g. "Qualified" (matched case-insensitively
    /// against the stage, not its id).
    pub stage: Option<String>,
    /// Salesperson to filter on: a login or display name. Omit for every
    /// salesperson your Odoo permissions let you see.
    pub user: Option<String>,
    /// Maximum leads to return (default 20, capped at 100).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct LeadGetInput {
    /// Odoo id of the lead or opportunity.
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadCreateInput {
    /// Subject of the lead — the one required field.
    pub name: String,
    /// Contact or company name.
    pub partner_name: Option<String>,
    pub email_from: Option<String>,
    pub phone: Option<String>,
    /// Longer description or qualification notes.
    pub description: Option<String>,
    /// Expected revenue in the company currency.
    pub expected_revenue: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadUpdateInput {
    /// Odoo id of the lead to update.
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
    /// One row per pipeline stage.
    Stage,
    /// One row per salesperson.
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadReportInput {
    /// Group the pipeline by stage or by salesperson.
    pub group_by: ReportGroupBy,
    /// Only count leads created on or after this date (YYYY-MM-DD).
    pub date_from: Option<String>,
    /// Only count leads created on or before this date (YYYY-MM-DD).
    pub date_to: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PartnerSearchInput {
    /// Free text matched against partner name, email and phone.
    pub query: String,
    /// Maximum partners to return (default 20, capped at 100).
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct PartnerGetInput {
    /// Odoo id of the partner.
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

/// Clamp a caller-supplied limit into the allowed range.
#[must_use]
pub fn resolve_limit(requested: Option<u32>) -> u32 {
    requested.map_or(DEFAULT_LIMIT, |l| l.clamp(1, MAX_LIMIT))
}
