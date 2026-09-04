//! The closing actions on `crm.lead`: won, lost, convert.
//!
//! Separated from the record tools in [`super`] because these do not write
//! fields — they invoke Odoo's own workflow methods, and the difference is the
//! whole reason they exist.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use crate::client::ModelCall;
use crate::format::text_artifact;
use crate::server::call::OdooCall;
use crate::tools::inputs::{LeadConvertInput, LeadMarkLostInput, LeadMarkWonInput};
use crate::tools::{TOOL_LEAD_CONVERT, TOOL_LEAD_MARK_LOST, TOOL_LEAD_MARK_WON};

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
            // Why: `convert_opportunity(partner_id)` — false lets Odoo match or
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
