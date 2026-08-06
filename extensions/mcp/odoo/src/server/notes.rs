//! Chatter and activity tools: `note_add` and `activity_list`.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::SearchOptions;
use crate::format::{empty_result, field_or_dash, text_artifact};
use crate::tools::inputs::{ActivityListInput, NoteAddInput, resolve_limit};
use crate::tools::{TOOL_ACTIVITY_LIST, TOOL_NOTE_ADD};

/// Fields read for a `mail.activity` row.
pub const ACTIVITY_FIELDS: [&str; 7] = [
    "id",
    "res_model",
    "res_name",
    "activity_type_id",
    "summary",
    "date_deadline",
    "user_id",
];

#[must_use]
pub fn activity_fields() -> Vec<String> {
    ACTIVITY_FIELDS.iter().map(|f| (*f).to_owned()).collect()
}

/// Activities assigned to the acting user, optionally narrowed.
///
/// `user_id = uid` is not a convenience filter — it is the tool's contract.
/// "List my activities" that quietly returned a colleague's would be worse
/// than useless, so the acting uid is baked in here and cannot be overridden
/// by input.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// exactly that; not part of the public API.
#[doc(hidden)]
#[must_use]
pub fn activity_domain(uid: i32, input: &ActivityListInput, today: &str) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = vec![serde_json::json!(["user_id", "=", uid])];
    if let Some(model) = input.model.as_deref().map(str::trim).filter(|m| !m.is_empty()) {
        domain.push(serde_json::json!(["res_model", "=", model]));
    }
    if input.overdue_only.unwrap_or(false) {
        domain.push(serde_json::json!(["date_deadline", "<", today]));
    }
    serde_json::Value::Array(domain)
}

#[must_use]
pub fn activity_row(record: &serde_json::Value) -> String {
    format!(
        "- **{}** — {} on {} ({}), due {}",
        field_or_dash(record, "summary"),
        field_or_dash(record, "activity_type_id"),
        field_or_dash(record, "res_name"),
        field_or_dash(record, "res_model"),
        field_or_dash(record, "date_deadline"),
    )
}

#[derive(Debug)]
pub struct NoteAddHandler {
    pub call: OdooCall,
}

impl McpToolHandler for NoteAddHandler {
    type Input = NoteAddInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_NOTE_ADD
    }

    fn description(&self) -> &'static str {
        "Log a note on an Odoo record's chatter."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let body = input.body.trim().to_owned();
            if body.is_empty() {
                return Err(McpError::invalid_params(
                    "A note body is required.".to_owned(),
                    None,
                ));
            }
            let message_id = call
                .client
                .message_post(&call.creds, &input.model, input.res_id, &body)
                .await?;

            let summary = format!(
                "Note posted on {} {} as {} (message {message_id})",
                input.model, input.res_id, call.creds.login
            );
            Ok((text_artifact("Note Logged", &summary), summary))
        }
    }
}

#[derive(Debug)]
pub struct ActivityListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for ActivityListHandler {
    type Input = ActivityListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_ACTIVITY_LIST
    }

    fn description(&self) -> &'static str {
        "List the Odoo activities assigned to you."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let today = chrono::Utc::now().date_naive().to_string();
            let options = SearchOptions {
                fields: activity_fields(),
                limit: resolve_limit(input.limit),
                order: Some("date_deadline asc".to_owned()),
            };
            let records = call
                .client
                .search_read(
                    &call.creds,
                    "mail.activity",
                    activity_domain(call.creds.uid, &input, &today),
                    &options,
                )
                .await?;

            let summary = format!("{} activity(ies) assigned to you", records.len());
            let body = if records.is_empty() {
                empty_result("activities")
            } else {
                records.iter().map(activity_row).collect::<Vec<_>>().join("\n")
            };
            Ok((text_artifact("Odoo Activities", &body), summary))
        }
    }
}
