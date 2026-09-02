//! `crm_lead_delete`: the one destructive `crm.lead` tool, kept apart from
//! [`crate::server::crm`] because it is destructive by design.
//!
//! It is a real Odoo `unlink`, not an archive. Archiving already exists via
//! `crm_lead_update {"active": false}`; a delete-named tool that only archived
//! would leave the governance blocklist demo protecting nothing. Odoo's own
//! ACL is the second gate — an `AccessError` surfaces as an access-denied
//! result. The lead is read first so a missing or invisible id fails cleanly
//! and its name survives into the summary of what is now gone.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use super::crm_shape::{LeadDeleted, LeadRow};
use crate::format::text_artifact;
use crate::tools::TOOL_LEAD_DELETE;
use crate::tools::inputs::LeadDeleteInput;

#[derive(Debug)]
pub struct LeadDeleteHandler {
    pub call: OdooCall,
}

impl McpToolHandler for LeadDeleteHandler {
    type Input = LeadDeleteInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_LEAD_DELETE
    }

    fn description(&self) -> &'static str {
        "Permanently delete a lead from Odoo CRM."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let mut records = call
                .client
                .read(&call.creds, "crm.lead", &[input.id], &["id", "name"])
                .await?;

            let Some(record) = records.pop() else {
                return Err(McpError::invalid_params(
                    format!(
                        "No lead with id {} is visible to your Odoo account, so there is \
                         nothing to delete.",
                        input.id
                    ),
                    None,
                ));
            };
            let lead: LeadRow = serde_json::from_value(record)
                .map_err(|e| McpError::internal_error(e.to_string(), None))?;

            let acknowledged = call
                .client
                .unlink(&call.creds, "crm.lead", &[input.id])
                .await?;
            if !acknowledged {
                return Err(McpError::internal_error(
                    format!("Odoo did not acknowledge deleting lead {}.", input.id),
                    None,
                ));
            }

            let deleted = LeadDeleted {
                id: input.id,
                name: lead.name,
                deleted: true,
            };
            let name = deleted.name.as_deref().unwrap_or("untitled");
            let summary = format!(
                "Deleted Odoo lead {} (\"{name}\") as {} — irreversible",
                deleted.id, call.creds.login
            );
            let body = format!(
                "Permanently deleted lead **[{}] {name}** from Odoo as `{}`. This was an \
                 `unlink`: the record, its chatter and its activities cannot be recovered.",
                deleted.id, call.creds.login
            );
            Ok((text_artifact("Lead Deleted", &body), summary))
        }
    }
}
