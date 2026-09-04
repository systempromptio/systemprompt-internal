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
pub mod sales;
pub mod work;

pub use knowledge::{
    AttachmentAddInput, AttachmentGetInput, AttachmentListInput, NoteAddInput, NoteListInput,
    NoteSearchInput,
};
pub use sales::{
    InvoiceGetInput, InvoiceListInput, SaleOrderCreateInput, SaleOrderGetInput, SaleOrderLineInput,
    SaleOrderListInput,
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
    pub open_only: Option<bool>,
    pub tag: Option<String>,
    pub sort: Option<LeadSort>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LeadSort {
    Created,
    Deadline,
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
    /// Link the lead to an existing customer record. Prefer this over
    /// `partner_name`: a lead carrying `partner_id` is joined to the contact
    /// database, and everything filed against that customer later — orders,
    /// invoices, chatter — hangs together. Use `partner_search` to find one,
    /// or `partner_create` when the customer is genuinely new.
    pub partner_id: Option<i64>,
    /// The contact as free text, for a prospect not yet worth a partner record.
    pub partner_name: Option<String>,
    pub email_from: Option<String>,
    pub phone: Option<String>,
    pub description: Option<String>,
    pub expected_revenue: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadUpdateInput {
    pub id: i64,
    /// Move the lead to this stage *by name* ("Qualified", "Won"), resolved
    /// against the pipeline. Prefer it over a raw `stage_id` in `fields`: the
    /// id is deployment-specific and guessing one moves the deal somewhere
    /// nobody asked for. Call `crm_stage_list` to see the names.
    pub stage: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PartnerCreateInput {
    pub name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub mobile: Option<String>,
    pub city: Option<String>,
    /// True for an organisation, false for an individual. Odoo defaults a new
    /// partner to an individual, so say so explicitly when creating a company.
    pub is_company: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct PartnerUpdateInput {
    pub id: i64,
    /// Field/value pairs written straight through to Odoo (`email`, `phone`,
    /// `street`, `city`).
    // JSON: passed through to Odoo's `write`, so the shape is Odoo's, not
    // ours — a fixed struct here would mean editing this crate every time a
    // deployment adds a custom field.
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct LeadMarkWonInput {
    pub id: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct LeadMarkLostInput {
    pub id: i64,
    /// Why it was lost, in the words the team would use. Recorded on the lead
    /// so the pipeline report can say more than "closed".
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct LeadConvertInput {
    pub id: i64,
    /// Attach the new opportunity to this existing customer. Omit to let Odoo
    /// create or match a partner from the lead's own contact fields.
    pub partner_id: Option<i64>,
}

/// No parameters: the stage list is the pipeline itself, and it is short.
#[expect(
    clippy::empty_structs_with_brackets,
    reason = "serde must accept the `{}` every MCP client sends; a unit struct rejects it"
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct StageListInput {}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct UserListInput {
    pub query: Option<String>,
    pub limit: Option<u32>,
}

/// No parameters: an Odoo deployment has a handful of activity types.
#[expect(
    clippy::empty_structs_with_brackets,
    reason = "serde must accept the `{}` every MCP client sends; a unit struct rejects it"
)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct ActivityTypeListInput {}
