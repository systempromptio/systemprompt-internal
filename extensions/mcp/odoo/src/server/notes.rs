//! The chatter tools: `note_add`, `note_list` and `note_search`.
//!
//! Odoo Community has no Knowledge app — that is an Enterprise module — so the
//! knowledge bank here is not a separate store. It is the chatter that already
//! hangs off business records: every `mail.message` is anchored to a
//! `(res_model, res_id)` pair, which means a note is never free-floating and
//! always answers "about what?".
//!
//! That anchoring is why [`note_search`](NoteSearchHandler) is the primary
//! retrieval tool rather than a convenience. `crm_lead_search` finds records by
//! their structured fields; this finds the *writing* people did about them, and
//! returns the record each hit belongs to so the caller can follow it.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::SearchOptions;
use crate::format::{empty_result, field, field_or_dash, text_artifact};
use crate::text::{html_to_text, snippet_around};
use crate::tools::inputs::{NoteAddInput, NoteListInput, NoteSearchInput, resolve_limit};
use crate::tools::{TOOL_NOTE_ADD, TOOL_NOTE_LIST, TOOL_NOTE_SEARCH};

// Why: one record's chatter is already scoped by the query, so the rows do not
// repeat the model and id the caller just supplied.
const THREAD_FIELDS: [&str; 5] = ["id", "author_id", "body", "date", "message_type"];

// Why: a search hit must carry its anchor — model, id and the record's display
// name — or the caller has found text it cannot navigate back to.
const SEARCH_FIELDS: [&str; 7] = [
    "id",
    "model",
    "res_id",
    "record_name",
    "author_id",
    "date",
    "body",
];

fn fields(names: &[&str]) -> Vec<String> {
    names.iter().map(|f| (*f).to_owned()).collect()
}

/// The chatter of exactly one record.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the anchor is an equality match on both halves; not part of the public API.
#[doc(hidden)]
#[must_use]
pub fn thread_domain(model: &str, res_id: i64) -> serde_json::Value {
    serde_json::json!([["model", "=", model], ["res_id", "=", res_id]])
}

/// The retrieval domain: free text over body and subject, optionally narrowed.
///
/// The two text leaves are OR'd because Odoo puts an emailed note's content in
/// `body` and its heading in `subject`, and a caller searching for a phrase
/// does not know which one holds it. Model and date filters AND onto that
/// group.
///
/// Exposed (behind `#[doc(hidden)]`) for the external test workspace; not part
/// of the public API.
#[doc(hidden)]
#[must_use]
pub fn search_domain(input: &NoteSearchInput) -> serde_json::Value {
    let pattern = format!("%{}%", input.query.trim());
    let mut domain: Vec<serde_json::Value> = vec![
        serde_json::json!("|"),
        serde_json::json!(["body", "ilike", pattern]),
        serde_json::json!(["subject", "ilike", pattern]),
    ];
    if let Some(model) = input
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
    {
        domain.push(serde_json::json!(["model", "=", model]));
    }
    if let Some(from) = input
        .date_from
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        domain.push(serde_json::json!(["date", ">=", from]));
    }
    if let Some(to) = input
        .date_to
        .as_deref()
        .map(str::trim)
        .filter(|d| !d.is_empty())
    {
        domain.push(serde_json::json!(["date", "<=", to]));
    }
    serde_json::Value::Array(domain)
}

/// One message in a record's chatter, as markdown.
#[doc(hidden)]
#[must_use]
pub fn thread_row(record: &serde_json::Value) -> String {
    let body =
        field(record, "body").map_or_else(|| "(empty note)".to_owned(), |html| html_to_text(&html));
    format!(
        "- **{}** — {} ({})\n  {body}",
        field_or_dash(record, "date"),
        field_or_dash(record, "author_id"),
        field_or_dash(record, "message_type"),
    )
}

/// One search hit: the anchor, then a snippet centred on the match.
#[doc(hidden)]
#[must_use]
pub fn search_row(record: &serde_json::Value, query: &str) -> String {
    let snippet = field(record, "body").map_or_else(
        || "(empty note)".to_owned(),
        |html| snippet_around(&html_to_text(&html), query),
    );
    format!(
        "- **{} {}** — {} · {} by {}\n  {snippet}",
        field_or_dash(record, "model"),
        field_or_dash(record, "res_id"),
        field_or_dash(record, "record_name"),
        field_or_dash(record, "date"),
        field_or_dash(record, "author_id"),
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
pub struct NoteListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for NoteListHandler {
    type Input = NoteListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_NOTE_LIST
    }

    fn description(&self) -> &'static str {
        "Read the chatter on one Odoo record."
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
                fields: fields(&THREAD_FIELDS),
                limit: resolve_limit(input.limit),
                order: Some("date desc".to_owned()),
            };
            let records = call
                .client
                .search_read(
                    &call.creds,
                    "mail.message",
                    thread_domain(&input.model, input.res_id),
                    &options,
                )
                .await?;

            let summary = format!(
                "{} message(s) on {} {}",
                records.len(),
                input.model,
                input.res_id
            );
            let body = if records.is_empty() {
                empty_result("chatter messages")
            } else {
                records
                    .iter()
                    .map(thread_row)
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((text_artifact("Odoo Record Chatter", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct NoteSearchHandler {
    pub call: OdooCall,
}

impl McpToolHandler for NoteSearchHandler {
    type Input = NoteSearchInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_NOTE_SEARCH
    }

    fn description(&self) -> &'static str {
        "Search every note in Odoo for what is known about a subject."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let query = input.query.trim().to_owned();
            if query.is_empty() {
                return Err(McpError::invalid_params(
                    "A search query is required — pass the subject you are looking for.".to_owned(),
                    None,
                ));
            }
            let options = SearchOptions {
                fields: fields(&SEARCH_FIELDS),
                limit: resolve_limit(input.limit),
                order: Some("date desc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, "mail.message", search_domain(&input), &options)
                .await?;

            let summary = format!("{} note(s) mention \"{query}\"", records.len());
            let body = if records.is_empty() {
                empty_result("notes")
            } else {
                records
                    .iter()
                    .map(|r| search_row(r, &query))
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok((text_artifact("Odoo Note Search", &body), summary))
        }
    }
}
