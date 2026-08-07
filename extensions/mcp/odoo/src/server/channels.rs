//! Discuss channels: `channel_list` and `channel_post`.
//!
//! The one place in this server where a message is not anchored to a business
//! record. A channel post is addressed to a room of people, so unlike
//! `note_add` it leaves no trace on any lead or partner — which is exactly why
//! the two are separate tools rather than one with a switch.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::SearchOptions;
use crate::format::{empty_result, field_or_dash, text_artifact};
use crate::tools::inputs::{ChannelListInput, ChannelPostInput, resolve_limit};
use crate::tools::{TOOL_CHANNEL_LIST, TOOL_CHANNEL_POST};

const CHANNEL_MODEL: &str = "discuss.channel";

// Why: member counts come from a related field Odoo computes per row. Asking
// for it is one column, not one query per channel, so it stays cheap.
const CHANNEL_FIELDS: [&str; 4] = ["id", "name", "channel_type", "member_count"];

/// Channels matching a name fragment, or all of them.
#[doc(hidden)]
#[must_use]
pub fn channel_domain(query: Option<&str>) -> serde_json::Value {
    query.map(str::trim).filter(|q| !q.is_empty()).map_or_else(
        || serde_json::json!([]),
        |q| serde_json::json!([["name", "ilike", format!("%{q}%")]]),
    )
}

/// One channel as a markdown row.
#[doc(hidden)]
#[must_use]
pub fn channel_row(record: &serde_json::Value) -> String {
    let id = record
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    format!(
        "- **[{id}] {}** — {} · {} member(s)",
        field_or_dash(record, "name"),
        field_or_dash(record, "channel_type"),
        field_or_dash(record, "member_count"),
    )
}

#[derive(Debug)]
pub struct ChannelListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for ChannelListHandler {
    type Input = ChannelListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_CHANNEL_LIST
    }

    fn description(&self) -> &'static str {
        "List the Discuss channels you can see."
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
                fields: CHANNEL_FIELDS.iter().map(|f| (*f).to_owned()).collect(),
                limit: resolve_limit(input.limit),
                order: Some("name asc".to_owned()),
            };
            let records = call
                .client
                .search_read(
                    &call.creds,
                    CHANNEL_MODEL,
                    channel_domain(input.query.as_deref()),
                    &options,
                )
                .await?;

            let summary = format!("{} channel(s) visible", records.len());
            let body = if records.is_empty() {
                empty_result("channels")
            } else {
                records
                    .iter()
                    .map(channel_row)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((text_artifact("Odoo Discuss Channels", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct ChannelPostHandler {
    pub call: OdooCall,
}

impl McpToolHandler for ChannelPostHandler {
    type Input = ChannelPostInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_CHANNEL_POST
    }

    fn description(&self) -> &'static str {
        "Post a message to a Discuss channel."
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
                    "A message body is required.".to_owned(),
                    None,
                ));
            }
            let message_id = call
                .client
                .message_post(&call.creds, CHANNEL_MODEL, input.channel_id, &body)
                .await?;

            let summary = format!(
                "Posted to channel {} as {} (message {message_id})",
                input.channel_id, call.creds.login
            );
            Ok((text_artifact("Message Posted", &summary), summary))
        }
    }
}
