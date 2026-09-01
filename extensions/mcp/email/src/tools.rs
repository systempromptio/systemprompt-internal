//! The tool `email_send` advertises.

use crate::draft::SendEmailInput;
use rmcp::model::{MetaObject, Tool};
use std::sync::Arc;
use systemprompt::mcp::{McpOutputSchema, default_tool_visibility, tool_ui_meta};
use systemprompt::models::artifacts::CliArtifact;

pub const SERVER_NAME: &str = "email";

// Why: The one tool. The name is a wire contract in two places beyond the
// client: `require_approval.patterns` matches it by substring, and the
// governance argument-conditioning rule addresses `SendEmailInput::to` by path.
pub const TOOL_EMAIL_SEND: &str = "email_send";

#[must_use]
// JSON: protocol boundary
pub fn input_schema() -> serde_json::Value {
    schemars::schema_for!(SendEmailInput).to_value()
}

#[must_use]
// JSON: protocol boundary
pub fn output_schema() -> serde_json::Value {
    <CliArtifact as McpOutputSchema>::validated_schema()
}

const DESCRIPTION: &str = "\
Send an email. This ALWAYS requires explicit human approval and cannot be made to send without it.

The first call returns a draft for review rather than sending anything: you get a preview card and \
a confirmation request. Present the draft to the user and let THEM decide — do not answer the \
confirmation on their behalf. Depending on the recipient and the caller's role, a second human may \
also have to approve it before it goes out.

Give `to`, `subject` and `body`. Set `reply_to` if replies should go somewhere other than the \
sending address. Set `res_model` and `res_id` together (e.g. \"crm.lead\" and the record id) to log \
the sent mail on that Odoo record's chatter — do this whenever the email concerns a lead or a \
partner, so the CRM stays the system of record.";

#[must_use]
pub fn list_tools() -> Vec<Tool> {
    let input_obj = input_schema()
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);
    let output_obj = output_schema()
        .as_object()
        .cloned()
        .unwrap_or_else(serde_json::Map::new);

    let mut tool = Tool::default();
    tool.name = TOOL_EMAIL_SEND.into();
    tool.title = Some("Send email".to_owned());
    tool.description = Some(DESCRIPTION.into());
    tool.input_schema = Arc::new(input_obj);
    tool.output_schema = Some(Arc::new(output_obj));
    // Why: Deliberately no `ToolAnnotations::read_only` — this tool has the most
    // side effects of anything on the instance.
    tool.meta = Some(MetaObject(tool_ui_meta(
        SERVER_NAME,
        &default_tool_visibility(),
    )));
    vec![tool]
}
