//! Project tasks: `task_list`, `task_create`, `task_update`.
//!
//! `task_create` takes a project *name* and refuses when nothing matches,
//! listing the projects the caller can see. Creating the project implicitly
//! would be the friendlier-looking choice and the wrong one: a typo would
//! silently spawn "Acme Rollout " beside "Acme Rollout", and nobody would
//! notice until a report came out short.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::SearchOptions;
use crate::format::{empty_result, field_or_dash, text_artifact};
use crate::resolve;
use crate::tools::inputs::{TaskCreateInput, TaskListInput, TaskUpdateInput, resolve_limit};
use crate::tools::{TOOL_TASK_CREATE, TOOL_TASK_LIST, TOOL_TASK_UPDATE};

const TASK_MODEL: &str = "project.task";

const TASK_FIELDS: [&str; 7] = [
    "id",
    "name",
    "project_id",
    "stage_id",
    "user_ids",
    "date_deadline",
    "priority",
];

#[doc(hidden)]
#[must_use]
pub fn task_domain(input: &TaskListInput, project_id: Option<i64>) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = Vec::new();
    if let Some(id) = project_id {
        domain.push(serde_json::json!(["project_id", "=", id]));
    }
    if let Some(query) = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        domain.push(serde_json::json!(["name", "ilike", format!("%{query}%")]));
    }
    if input.open_only.unwrap_or(true) {
        domain.push(serde_json::json!(["stage_id.fold", "=", false]));
    }
    serde_json::Value::Array(domain)
}

#[doc(hidden)]
#[must_use]
pub fn task_row(record: &serde_json::Value) -> String {
    let id = record
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    let assignees = record
        .get("user_ids")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    format!(
        "- **[{id}] {}** — {} · {} · due {} · {assignees} assignee(s)",
        field_or_dash(record, "name"),
        field_or_dash(record, "project_id"),
        field_or_dash(record, "stage_id"),
        field_or_dash(record, "date_deadline"),
    )
}

#[derive(Debug)]
pub struct TaskListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for TaskListHandler {
    type Input = TaskListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_TASK_LIST
    }

    fn description(&self) -> &'static str {
        "List project tasks, open ones by default."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let project_id = match input
                .project
                .as_deref()
                .map(str::trim)
                .filter(|p| !p.is_empty())
            {
                Some(name) => Some(resolve::project_id(&call.client, &call.creds, name).await?),
                None => None,
            };
            let options = SearchOptions {
                fields: TASK_FIELDS.iter().map(|f| (*f).to_owned()).collect(),
                limit: resolve_limit(input.limit),
                order: Some("date_deadline asc, priority desc".to_owned()),
            };
            let records = call
                .client
                .search_read(
                    &call.creds,
                    TASK_MODEL,
                    task_domain(&input, project_id),
                    &options,
                )
                .await?;

            let summary = format!("{} task(s) matched", records.len());
            let body = if records.is_empty() {
                empty_result("tasks")
            } else {
                records.iter().map(task_row).collect::<Vec<_>>().join("\n")
            };
            Ok((text_artifact("Odoo Tasks", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct TaskCreateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for TaskCreateHandler {
    type Input = TaskCreateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_TASK_CREATE
    }

    fn description(&self) -> &'static str {
        "Create a task in an existing project."
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
                    "A task name is required.".to_owned(),
                    None,
                ));
            }
            let project_id = resolve::project_id(&call.client, &call.creds, &input.project).await?;

            let mut values = serde_json::Map::new();
            values.insert("name".to_owned(), serde_json::json!(name));
            values.insert("project_id".to_owned(), serde_json::json!(project_id));
            if let Some(who) = input
                .user
                .as_deref()
                .map(str::trim)
                .filter(|u| !u.is_empty())
            {
                let user_id = resolve::user_id(&call.client, &call.creds, who).await?;
                // Why: project.task assigns through a many2many, so a single
                // assignee is still written as a set-replacement command.
                values.insert(
                    "user_ids".to_owned(),
                    serde_json::json!([[6, 0, [user_id]]]),
                );
            }
            if let Some(description) = input
                .description
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
            {
                values.insert("description".to_owned(), serde_json::json!(description));
            }
            if let Some(deadline) = input
                .date_deadline
                .as_deref()
                .map(str::trim)
                .filter(|d| !d.is_empty())
            {
                values.insert("date_deadline".to_owned(), serde_json::json!(deadline));
            }

            let id = call
                .client
                .create(&call.creds, TASK_MODEL, serde_json::Value::Object(values))
                .await?;

            let summary = format!("Created task {id} \"{name}\" in {}", input.project.trim());
            Ok((text_artifact("Task Created", &summary), summary))
        }
    }
}

#[derive(Debug)]
pub struct TaskUpdateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for TaskUpdateHandler {
    type Input = TaskUpdateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_TASK_UPDATE
    }

    fn description(&self) -> &'static str {
        "Update fields on an existing project task."
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
                    TASK_MODEL,
                    input.id,
                    serde_json::Value::Object(input.fields),
                )
                .await?;

            if !written {
                return Err(McpError::internal_error(
                    format!("Odoo did not acknowledge the update to task {}.", input.id),
                    None,
                ));
            }
            let summary = format!("Updated task {} ({})", input.id, changed.join(", "));
            Ok((text_artifact("Task Updated", &summary), summary))
        }
    }
}
