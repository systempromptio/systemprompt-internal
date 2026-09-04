//! `res.partner` writes: `partner_create` and `partner_update`.
//!
//! Kept apart from [`super::partner`] so the read path and the write path are
//! not read as one surface: searching customers is something every skill does,
//! creating one is not.
//!
//! Creating a customer is how a lead stops being an island. A lead carrying
//! only `partner_name` as free text is invisible to the contact database —
//! nothing joins it to the orders, invoices or chatter filed against the same
//! company under a slightly different spelling.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::format::text_artifact;
use crate::tools::inputs::{PartnerCreateInput, PartnerUpdateInput};
use crate::tools::{TOOL_PARTNER_CREATE, TOOL_PARTNER_UPDATE};

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
pub struct PartnerCreateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for PartnerCreateHandler {
    type Input = PartnerCreateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_PARTNER_CREATE
    }

    fn description(&self) -> &'static str {
        "Create a customer or contact in Odoo."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let name = input.name.trim().to_owned();
            if name.is_empty() {
                return Err(McpError::invalid_params(
                    "A customer needs a name.".to_owned(),
                    None,
                ));
            }

            let mut values = serde_json::Map::new();
            values.insert("name".to_owned(), serde_json::json!(name));
            insert_opt(&mut values, "email", input.email);
            insert_opt(&mut values, "phone", input.phone);
            insert_opt(&mut values, "mobile", input.mobile);
            insert_opt(&mut values, "city", input.city);
            if let Some(is_company) = input.is_company {
                values.insert("is_company".to_owned(), serde_json::json!(is_company));
            }

            let id = call
                .client
                .create(
                    &call.creds,
                    "res.partner",
                    serde_json::Value::Object(values),
                )
                .await?;

            let summary = format!("Created Odoo customer {id} ({name})");
            let body = format!(
                "Created customer **[{id}] {name}** in Odoo, by `{}`.\n\nPass `partner_id: \
                 {id}` to crm_lead_create so the lead is linked to this record.",
                call.creds.login
            );
            Ok((text_artifact("Customer Created", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct PartnerUpdateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for PartnerUpdateHandler {
    type Input = PartnerUpdateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_PARTNER_UPDATE
    }

    fn description(&self) -> &'static str {
        "Update fields on an existing Odoo customer."
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
                    "res.partner",
                    input.id,
                    serde_json::Value::Object(input.fields),
                )
                .await?;

            if !written {
                return Err(McpError::internal_error(
                    format!(
                        "Odoo did not acknowledge the update to customer {}.",
                        input.id
                    ),
                    None,
                ));
            }
            let summary = format!(
                "Updated Odoo customer {} ({})",
                input.id,
                changed.join(", ")
            );
            Ok((text_artifact("Customer Updated", &summary), summary))
        }
    }
}
