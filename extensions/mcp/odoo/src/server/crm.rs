//! The four `crm.lead` record tools: search, get, create, update.
//!
//! Aggregation lives next door in [`crate::server::report`].

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::{OdooCall, lead_fields};
use crate::client::{ModelCall, SearchOptions};
use crate::format::{detail_lines, empty_result, text_artifact};
use crate::resolve;
use crate::tools::inputs::{
    LeadConvertInput, LeadCreateInput, LeadGetInput, LeadMarkLostInput, LeadMarkWonInput,
    LeadSearchInput, LeadUpdateInput, resolve_limit,
};
use crate::tools::{
    TOOL_LEAD_CONVERT, TOOL_LEAD_CREATE, TOOL_LEAD_GET, TOOL_LEAD_MARK_LOST, TOOL_LEAD_MARK_WON,
    TOOL_LEAD_SEARCH, TOOL_LEAD_UPDATE,
};

use super::crm_shape::LEAD_LABELS;
pub use super::crm_shape::{
    LeadDeleted, LeadRow, TagRow, attach_tag_names, lead_domain, lead_order, lead_row, lead_rows,
    lead_table, odoo, tag_ids_of, tag_names,
};

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
                order: Some(lead_order(&input)),
            };
            let records = call
                .client
                .search_read(&call.creds, "crm.lead", lead_domain(&input), &options)
                .await?;
            let mut rows = lead_rows(&records);

            let tag_ids = tag_ids_of(&rows);
            if !tag_ids.is_empty() {
                let tags = call
                    .client
                    .read(&call.creds, "crm.tag", &tag_ids, &["id", "name"])
                    .await?;
                attach_tag_names(&mut rows, &tag_names(&tags));
            }

            let summary = if rows.is_empty() {
                empty_result("leads")
            } else {
                format!("{} lead(s) matched in Odoo", rows.len())
            };
            Ok((CliArtifact::table(lead_table(&rows)), summary))
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
            if let Some(partner_id) = input.partner_id {
                values.insert("partner_id".to_owned(), serde_json::json!(partner_id));
            }
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
            let mut fields = input.fields;
            // Why: a stage NAME is what a person says, and resolving it here is
            // the difference between moving the right deal and moving whichever
            // one happened to carry the guessed id.
            if let Some(stage) = input
                .stage
                .as_deref()
                .map(str::trim)
                .filter(|s| !s.is_empty())
            {
                let stage_id = resolve::stage_id(&call.client, &call.creds, stage)
                    .await?
                    .ok_or_else(|| {
                        McpError::invalid_params(
                            format!(
                                "No pipeline stage named {stage:?}. Call crm_stage_list to see \
                                 the stages this Odoo defines."
                            ),
                            None,
                        )
                    })?;
                fields.insert("stage_id".to_owned(), serde_json::json!(stage_id));
            }

            if fields.is_empty() {
                return Err(McpError::invalid_params(
                    "No fields to update — pass `stage`, or at least one field/value pair."
                        .to_owned(),
                    None,
                ));
            }
            let changed: Vec<String> = fields.keys().cloned().collect();
            let written = call
                .client
                .write(
                    &call.creds,
                    "crm.lead",
                    input.id,
                    serde_json::Value::Object(fields),
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

// Why: Odoo's closing actions do more than set a number — they move the stage,
// stamp the close date and fire the automations a deployment hangs off a win.
// Writing `probability` by hand looked equivalent and left the pipeline report
// disagreeing with the dashboard.
async fn run_lead_action(
    call: &OdooCall,
    id: i64,
    method: &str,
) -> Result<serde_json::Value, McpError> {
    call.client
        .execute_kw(
            &call.creds,
            ModelCall {
                model: "crm.lead",
                method,
                args: serde_json::json!([[id]]),
                kwargs: serde_json::json!({}),
            },
        )
        .await
        .map_err(McpError::from)
}

#[derive(Debug)]
pub struct LeadMarkWonHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadMarkWonHandler {
    type Input = LeadMarkWonInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_MARK_WON
    }

    fn description(&self) -> &'static str {
        "Close an Odoo lead as won."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            run_lead_action(&call, input.id, "action_set_won_rainbowman").await?;
            let summary = format!("Marked Odoo lead {} won as {}", input.id, call.creds.login);
            let body = format!(
                "Lead **[{}]** is now **won**, closed by `{}`.",
                input.id, call.creds.login
            );
            Ok((text_artifact("Deal Won", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct LeadMarkLostHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadMarkLostHandler {
    type Input = LeadMarkLostInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_MARK_LOST
    }

    fn description(&self) -> &'static str {
        "Close an Odoo lead as lost, optionally recording why."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            // Why: the reason goes to chatter, not to `description`. Writing
            // it into the field would overwrite whatever the team had already
            // written on the lead — the close would silently cost them the
            // history that explains it. Posted before the close, because
            // `action_set_lost` may archive the row out from under a later
            // write.
            if let Some(reason) = input
                .reason
                .as_deref()
                .map(str::trim)
                .filter(|r| !r.is_empty())
            {
                call.client
                    .message_post(
                        &call.creds,
                        "crm.lead",
                        input.id,
                        &format!("Marked lost: {reason}"),
                    )
                    .await?;
            }
            run_lead_action(&call, input.id, "action_set_lost").await?;

            let summary = format!("Marked Odoo lead {} lost as {}", input.id, call.creds.login);
            let body = match input.reason.as_deref() {
                Some(reason) if !reason.trim().is_empty() => format!(
                    "Lead **[{}]** is now **lost**, closed by `{}`.\n\nReason: {}",
                    input.id,
                    call.creds.login,
                    reason.trim()
                ),
                _ => format!(
                    "Lead **[{}]** is now **lost**, closed by `{}`.",
                    input.id, call.creds.login
                ),
            };
            Ok((text_artifact("Deal Lost", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct LeadConvertHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadConvertHandler {
    type Input = LeadConvertInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_CONVERT
    }

    fn description(&self) -> &'static str {
        "Convert an Odoo lead into an opportunity."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            // `convert_opportunity(partner_id)` — false lets Odoo match or
            // create the partner from the lead's own contact fields.
            let partner = input
                .partner_id
                .map_or_else(|| serde_json::json!(false), |id| serde_json::json!(id));
            call.client
                .execute_kw(
                    &call.creds,
                    ModelCall {
                        model: "crm.lead",
                        method: "convert_opportunity",
                        args: serde_json::json!([[input.id], partner]),
                        kwargs: serde_json::json!({}),
                    },
                )
                .await?;

            let summary = format!("Converted Odoo lead {} to an opportunity", input.id);
            Ok((text_artifact("Lead Converted", &summary), summary))
        }
    }
}
