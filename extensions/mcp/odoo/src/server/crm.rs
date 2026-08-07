//! The four `crm.lead` record tools: search, get, create, update.
//!
//! Aggregation lives next door in [`crate::server::report`].

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::{OdooCall, lead_fields};
use crate::client::SearchOptions;
use crate::format::{detail_lines, empty_result, text_artifact};
use crate::tools::inputs::{
    LeadCreateInput, LeadGetInput, LeadSearchInput, LeadUpdateInput, resolve_limit,
};
use crate::tools::{TOOL_LEAD_CREATE, TOOL_LEAD_GET, TOOL_LEAD_SEARCH, TOOL_LEAD_UPDATE};

use super::crm_shape::LEAD_LABELS;
pub use super::crm_shape::{lead_domain, lead_row};

#[derive(Debug)]
pub struct LeadSearchHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadSearchHandler {
    type Input = LeadSearchInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_SEARCH
    }

    fn description(&self) -> &'static str {
        "Search leads and opportunities in Odoo CRM."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let options = SearchOptions {
                fields: lead_fields(),
                limit: resolve_limit(input.limit),
                order: Some("create_date desc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, "crm.lead", lead_domain(&input), &options)
                .await?;

            let summary = format!("{} lead(s) matched in Odoo", records.len());
            let body = if records.is_empty() {
                empty_result("leads")
            } else {
                records.iter().map(lead_row).collect::<Vec<_>>().join("\n")
            };
            Ok((text_artifact("Odoo CRM Leads", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct LeadGetHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadGetHandler {
    type Input = LeadGetInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_GET
    }

    fn description(&self) -> &'static str {
        "Read one lead or opportunity by Odoo id."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            // Why: the detail view adds `description`, which the list views
            // deliberately omit — it is unbounded free text and would blow
            // out a search result.
            let mut fields: Vec<&str> = super::call::LEAD_FIELDS.to_vec();
            fields.push("description");
            let mut records = call
                .client
                .read(&call.creds, "crm.lead", &[input.id], &fields)
                .await?;

            let Some(record) = records.pop() else {
                return Err(McpError::invalid_params(
                    format!(
                        "No lead with id {} is visible to your Odoo account.",
                        input.id
                    ),
                    None,
                ));
            };

            let mut body = detail_lines(&record, &LEAD_LABELS);
            if let Some(description) = crate::format::field(&record, "description") {
                body.push_str(&format!("\n\n{description}"));
            }
            let summary = format!("Lead {} read from Odoo", input.id);
            Ok((text_artifact("Odoo Lead", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct LeadCreateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadCreateHandler {
    type Input = LeadCreateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_CREATE
    }

    fn description(&self) -> &'static str {
        "Create a lead in Odoo CRM."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let mut values = serde_json::Map::new();
            values.insert("name".to_owned(), serde_json::json!(input.name));
            insert_opt(&mut values, "partner_name", input.partner_name);
            insert_opt(&mut values, "email_from", input.email_from);
            insert_opt(&mut values, "phone", input.phone);
            insert_opt(&mut values, "description", input.description);
            if let Some(revenue) = input.expected_revenue {
                values.insert("expected_revenue".to_owned(), serde_json::json!(revenue));
            }

            let id = call
                .client
                .create(&call.creds, "crm.lead", serde_json::Value::Object(values))
                .await?;

            let summary = format!("Created Odoo lead {id} as {}", call.creds.login);
            let body = format!(
                "Created lead **[{id}] {}** in Odoo, owned by `{}`.",
                input.name, call.creds.login
            );
            Ok((text_artifact("Lead Created", &body), summary))
        }
    }
}

fn insert_opt(
    values: &mut serde_json::Map<String, serde_json::Value>,
    key: &str,
    value: Option<String>,
) {
    if let Some(value) = value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty()) {
        values.insert(key.to_owned(), serde_json::json!(value));
    }
}

#[derive(Debug)]
pub struct LeadUpdateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadUpdateHandler {
    type Input = LeadUpdateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_UPDATE
    }

    fn description(&self) -> &'static str {
        "Update fields on an existing Odoo lead."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            if input.fields.is_empty() {
                return Err(McpError::invalid_params(
                    "No fields to update — pass at least one field/value pair.".to_owned(),
                    None,
                ));
            }
            let changed: Vec<String> = input.fields.keys().cloned().collect();
            let written = call
                .client
                .write(
                    &call.creds,
                    "crm.lead",
                    input.id,
                    serde_json::Value::Object(input.fields),
                )
                .await?;

            if !written {
                return Err(McpError::internal_error(
                    format!("Odoo did not acknowledge the update to lead {}.", input.id),
                    None,
                ));
            }
            let summary = format!("Updated Odoo lead {} ({})", input.id, changed.join(", "));
            Ok((text_artifact("Lead Updated", &summary), summary))
        }
    }
}
