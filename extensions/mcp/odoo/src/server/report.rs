//! `crm_lead_report` — pipeline aggregation via Odoo's `read_group`.
//!
//! The alternative a model reaches for unprompted is "search every lead, then
//! count them", which pulls the whole pipeline through the context window to
//! produce six numbers. This does the arithmetic in Postgres, where it belongs.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::GroupQuery;
use crate::format::{empty_result, field_or_dash, text_artifact};
use crate::tools::TOOL_LEAD_REPORT;
use crate::tools::inputs::{LeadReportInput, ReportGroupBy};

// Why: the read_group aggregation fields. `__count` comes back regardless, so
// only the revenue sum has to be asked for.
const AGGREGATES: [&str; 1] = ["expected_revenue:sum"];

#[must_use]
const fn group_field(group_by: ReportGroupBy) -> &'static str {
    match group_by {
        ReportGroupBy::Stage => "stage_id",
        ReportGroupBy::User => "user_id",
    }
}

/// Build the creation-date window. An open-ended range is the normal case and
/// produces an empty domain rather than a sentinel date.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// each of the four from/to combinations; not part of the public API.
#[doc(hidden)]
#[must_use]
pub fn report_domain(input: &LeadReportInput) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = Vec::new();
    if let Some(from) = input.date_from.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        domain.push(serde_json::json!(["create_date", ">=", from]));
    }
    if let Some(to) = input.date_to.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        domain.push(serde_json::json!(["create_date", "<=", to]));
    }
    serde_json::Value::Array(domain)
}

/// Render one `read_group` bucket. Odoo names the count `__count` and the
/// summed field after the field itself, not after the aggregate expression.
#[doc(hidden)]
#[must_use]
pub fn group_row(record: &serde_json::Value, group_key: &str) -> String {
    let count = record
        .get("__count")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    let revenue = record
        .get("expected_revenue")
        .and_then(serde_json::Value::as_f64)
        .unwrap_or_default();
    format!(
        "- **{}** — {count} lead(s), expected revenue {revenue:.2}",
        field_or_dash(record, group_key)
    )
}

#[derive(Debug)]
pub struct LeadReportHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadReportHandler {
    type Input = LeadReportInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_REPORT
    }

    fn description(&self) -> &'static str {
        "Aggregate the Odoo CRM pipeline by stage or salesperson."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let group_key = group_field(input.group_by);
            let records = call
                .client
                .read_group(&call.creds, "crm.lead", GroupQuery {
                    domain: report_domain(&input),
                    fields: &AGGREGATES,
                    group_by: &[group_key],
                })
                .await?;

            let total: i64 = records
                .iter()
                .filter_map(|r| r.get("__count").and_then(serde_json::Value::as_i64))
                .sum();
            let summary =
                format!("{total} lead(s) across {} group(s) by {group_key}", records.len());
            let body = if records.is_empty() {
                empty_result("leads")
            } else {
                records
                    .iter()
                    .map(|r| group_row(r, group_key))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((text_artifact("Odoo Pipeline Report", &body), summary))
        }
    }
}
