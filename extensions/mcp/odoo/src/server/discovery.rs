//! The lookups that turn a name into an id: `crm_stage_list`, `user_list`,
//! `activity_type_list`.
//!
//! These are here because their absence had a cost. Moving a lead required a
//! numeric `stage_id` and nothing in this server would tell you what the
//! stages were, so the id was either guessed or carried in from somewhere
//! else. A guessed stage id is not a failed call — it is a deal quietly moved
//! to the wrong column.
//!
//! All three are read-only, cheap, and scoped by the caller's own Odoo record
//! rules like every other tool here.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::SearchOptions;
use crate::format::{empty_result, field_or_dash, text_artifact};
use crate::tools::inputs::{ActivityTypeListInput, StageListInput, UserListInput, resolve_limit};
use crate::tools::{TOOL_ACTIVITY_TYPE_LIST, TOOL_STAGE_LIST, TOOL_USER_LIST};

#[derive(Debug)]
pub struct StageListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for StageListHandler {
    type Input = StageListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_STAGE_LIST
    }

    fn description(&self) -> &'static str {
        "List the CRM pipeline's stages in order."
    }

    fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let options = SearchOptions {
                fields: vec![
                    "id".to_owned(),
                    "name".to_owned(),
                    "sequence".to_owned(),
                    "is_won".to_owned(),
                    "fold".to_owned(),
                ],
                limit: 100,
                order: Some("sequence asc, id asc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, "crm.stage", serde_json::json!([]), &options)
                .await?;

            if records.is_empty() {
                let msg = empty_result("pipeline stages");
                return Ok((text_artifact("Pipeline Stages", &msg), msg));
            }

            let mut body = String::from("Pipeline stages, in order:\n\n");
            for record in &records {
                let won = record
                    .get("is_won")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                body.push_str(&format!(
                    "- **[{}] {}**{}\n",
                    field_or_dash(record, "id"),
                    field_or_dash(record, "name"),
                    if won { " — counts as won" } else { "" }
                ));
            }
            let summary = format!("{} pipeline stage(s)", records.len());
            Ok((text_artifact("Pipeline Stages", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct UserListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for UserListHandler {
    type Input = UserListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_USER_LIST
    }

    fn description(&self) -> &'static str {
        "List the Odoo users work can be assigned to."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let domain = input
                .query
                .as_deref()
                .map(str::trim)
                .filter(|q| !q.is_empty())
                .map_or_else(
                    || serde_json::json!([]),
                    |query| {
                        let pattern = format!("%{query}%");
                        serde_json::json!([
                            "|",
                            ["login", "ilike", pattern],
                            ["name", "ilike", pattern]
                        ])
                    },
                );
            let options = SearchOptions {
                fields: vec!["id".to_owned(), "name".to_owned(), "login".to_owned()],
                limit: resolve_limit(input.limit),
                order: Some("name asc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, "res.users", domain, &options)
                .await?;

            if records.is_empty() {
                let msg = empty_result("Odoo users");
                return Ok((text_artifact("Odoo Users", &msg), msg));
            }

            let mut body = String::new();
            for record in &records {
                body.push_str(&format!(
                    "- **[{}] {}** — `{}`\n",
                    field_or_dash(record, "id"),
                    field_or_dash(record, "name"),
                    field_or_dash(record, "login"),
                ));
            }
            let summary = format!("{} Odoo user(s)", records.len());
            Ok((text_artifact("Odoo Users", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct ActivityTypeListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for ActivityTypeListHandler {
    type Input = ActivityTypeListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_ACTIVITY_TYPE_LIST
    }

    fn description(&self) -> &'static str {
        "List the activity types this Odoo defines."
    }

    fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let options = SearchOptions {
                fields: vec!["id".to_owned(), "name".to_owned()],
                limit: 100,
                order: Some("sequence asc, id asc".to_owned()),
            };
            let records = call
                .client
                .search_read(
                    &call.creds,
                    "mail.activity.type",
                    serde_json::json!([]),
                    &options,
                )
                .await?;

            if records.is_empty() {
                let msg = empty_result("activity types");
                return Ok((text_artifact("Activity Types", &msg), msg));
            }

            let mut body = String::new();
            for record in &records {
                body.push_str(&format!(
                    "- **[{}] {}**\n",
                    field_or_dash(record, "id"),
                    field_or_dash(record, "name"),
                ));
            }
            let summary = format!("{} activity type(s)", records.len());
            Ok((text_artifact("Activity Types", &body), summary))
        }
    }
}
