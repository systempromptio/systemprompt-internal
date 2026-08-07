//! `res.partner` tools: search and get.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::{OdooCall, PARTNER_FIELDS, partner_fields};
use crate::client::SearchOptions;
use crate::format::{detail_lines, empty_result, field_or_dash, text_artifact};
use crate::tools::inputs::{PartnerGetInput, PartnerSearchInput, resolve_limit};
use crate::tools::{TOOL_PARTNER_GET, TOOL_PARTNER_SEARCH};

const PARTNER_LABELS: [(&str, &str); 7] = [
    ("name", "Name"),
    ("email", "Email"),
    ("phone", "Phone"),
    ("mobile", "Mobile"),
    ("city", "City"),
    ("country_id", "Country"),
    ("category_id", "Tags"),
];

/// Free-text partner search across the three columns anyone actually searches
/// by. Prefix `"|"` operators make it an OR.
#[doc(hidden)]
#[must_use]
pub fn partner_domain(query: &str) -> serde_json::Value {
    serde_json::json!([
        "|",
        "|",
        ["name", "ilike", query],
        ["email", "ilike", query],
        ["phone", "ilike", query]
    ])
}

#[derive(Debug)]
pub struct PartnerSearchHandler {
    pub call: OdooCall,
}

impl McpToolHandler for PartnerSearchHandler {
    type Input = PartnerSearchInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_PARTNER_SEARCH
    }

    fn description(&self) -> &'static str {
        "Search Odoo partners by name, email or phone."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let query = input.query.trim().to_owned();
            if query.is_empty() {
                return Err(McpError::invalid_params(
                    "A search query is required — pass a name, email or phone fragment.".to_owned(),
                    None,
                ));
            }
            let options = SearchOptions {
                fields: partner_fields(),
                limit: resolve_limit(input.limit),
                order: Some("name asc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, "res.partner", partner_domain(&query), &options)
                .await?;

            let summary = format!("{} partner(s) matched \"{query}\"", records.len());
            let body = if records.is_empty() {
                empty_result("partners")
            } else {
                records
                    .iter()
                    .map(|r| {
                        let id = r
                            .get("id")
                            .and_then(serde_json::Value::as_i64)
                            .unwrap_or_default();
                        format!(
                            "- **[{id}] {}** — {} · {}",
                            field_or_dash(r, "name"),
                            field_or_dash(r, "email"),
                            field_or_dash(r, "phone")
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((text_artifact("Odoo Partners", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct PartnerGetHandler {
    pub call: OdooCall,
}

impl McpToolHandler for PartnerGetHandler {
    type Input = PartnerGetInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_PARTNER_GET
    }

    fn description(&self) -> &'static str {
        "Read one Odoo partner by id."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let fields: Vec<&str> = PARTNER_FIELDS.to_vec();
            let mut records = call
                .client
                .read(&call.creds, "res.partner", &[input.id], &fields)
                .await?;

            let Some(record) = records.pop() else {
                return Err(McpError::invalid_params(
                    format!(
                        "No partner with id {} is visible to your Odoo account.",
                        input.id
                    ),
                    None,
                ));
            };

            let summary = format!("Partner {} read from Odoo", input.id);
            Ok((
                text_artifact("Odoo Partner", &detail_lines(&record, &PARTNER_LABELS)),
                summary,
            ))
        }
    }
}
