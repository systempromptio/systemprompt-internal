//! Activities: `activity_list`, `activity_create`, `activity_complete`.
//!
//! Activities are the other half of a record's history: chatter records what
//! was said, `mail.activity` records what someone undertook to do next. Split
//! from the chatter tools because the anchoring is different — an activity is
//! addressed to a *person*.
//!
//! Completing one is not a delete. Odoo's `action_feedback` closes the activity
//! *and* writes the feedback to the record's chatter, so the fact that the work
//! happened outlives the reminder that it was due. That is why
//! `activity_complete` takes feedback worth keeping rather than a confirmation
//! flag.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::{ModelCall, SearchOptions};
use crate::format::{empty_result, field_or_dash, text_artifact};
use crate::resolve;
use crate::tools::inputs::{
    ActivityCompleteInput, ActivityCreateInput, ActivityListInput, resolve_limit,
};
use crate::tools::{TOOL_ACTIVITY_COMPLETE, TOOL_ACTIVITY_CREATE, TOOL_ACTIVITY_LIST};

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
    if let Some(model) = input
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
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
                records
                    .iter()
                    .map(activity_row)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((text_artifact("Odoo Activities", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct ActivityCreateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for ActivityCreateHandler {
    type Input = ActivityCreateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_ACTIVITY_CREATE
    }

    fn description(&self) -> &'static str {
        "Schedule an activity on an Odoo record."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let summary_text = input.summary.trim().to_owned();
            if summary_text.is_empty() {
                return Err(McpError::invalid_params(
                    "An activity summary is required — say what is to be done.".to_owned(),
                    None,
                ));
            }

            // Why: defaulting the assignee to the caller rather than leaving it
            // unset. An unassigned activity appears on nobody's list, which
            // makes "schedule a follow-up" quietly do nothing.
            let user_id = match input
                .user
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
            {
                Some(who) => resolve::user_id(&call.client, &call.creds, who).await?,
                None => i64::from(call.creds.uid),
            };
            let (type_id, res_model_id) = tokio::try_join!(
                resolve::activity_type_id(&call.client, &call.creds),
                resolve::model_id(&call.client, &call.creds, &input.model),
            )?;

            let mut values = serde_json::Map::new();
            values.insert("res_model_id".to_owned(), serde_json::json!(res_model_id));
            values.insert("res_model".to_owned(), serde_json::json!(input.model));
            values.insert("res_id".to_owned(), serde_json::json!(input.res_id));
            values.insert("summary".to_owned(), serde_json::json!(summary_text));
            values.insert(
                "date_deadline".to_owned(),
                serde_json::json!(input.date_deadline.trim()),
            );
            values.insert("user_id".to_owned(), serde_json::json!(user_id));
            values.insert("activity_type_id".to_owned(), serde_json::json!(type_id));
            if let Some(note) = input
                .note
                .as_deref()
                .map(str::trim)
                .filter(|n| !n.is_empty())
            {
                values.insert("note".to_owned(), serde_json::json!(note));
            }

            let id = call
                .client
                .create(
                    &call.creds,
                    "mail.activity",
                    serde_json::Value::Object(values),
                )
                .await?;

            let summary = format!(
                "Scheduled activity {id} on {} {} for {}, due {}",
                input.model,
                input.res_id,
                input.user.as_deref().unwrap_or(&call.creds.login),
                input.date_deadline.trim()
            );
            Ok((text_artifact("Activity Scheduled", &summary), summary))
        }
    }
}

#[derive(Debug)]
pub struct ActivityCompleteHandler {
    pub call: OdooCall,
}

impl McpToolHandler for ActivityCompleteHandler {
    type Input = ActivityCompleteInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_ACTIVITY_COMPLETE
    }

    fn description(&self) -> &'static str {
        "Mark an Odoo activity done, logging the outcome to the record."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let feedback = input
                .feedback
                .as_deref()
                .map(str::trim)
                .filter(|f| !f.is_empty())
                .unwrap_or("Done");

            call.client
                .execute_kw(
                    &call.creds,
                    ModelCall {
                        model: "mail.activity",
                        method: "action_feedback",
                        args: serde_json::json!([[input.activity_id]]),
                        kwargs: serde_json::json!({ "feedback": feedback }),
                    },
                )
                .await?;

            let summary = format!(
                "Activity {} marked done by {}; feedback logged to the record's chatter",
                input.activity_id, call.creds.login
            );
            Ok((text_artifact("Activity Completed", &summary), summary))
        }
    }
}
