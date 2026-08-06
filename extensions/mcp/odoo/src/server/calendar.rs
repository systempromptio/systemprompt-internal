//! Calendar events: `calendar_event_list` and `calendar_event_create`.
//!
//! Odoo stores every datetime in UTC and expects `YYYY-MM-DD HH:MM:SS`, with no
//! zone marker. Callers reach for ISO 8601 with a `T` and often a `Z`, so
//! [`normalize_datetime`] accepts both rather than failing on the shape almost
//! everything else in the world uses.
//!
//! An event with no end is the common case in conversation ("book an hour
//! tomorrow at ten") and an error in Odoo, so a missing `stop` is derived from
//! `duration_hours`, defaulting to one hour.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::OdooCall;
use crate::client::SearchOptions;
use crate::format::{empty_result, field_or_dash, text_artifact};
use crate::text::html_to_text;
use crate::tools::inputs::{CalendarEventCreateInput, CalendarEventListInput, resolve_limit};
use crate::tools::{TOOL_CALENDAR_EVENT_CREATE, TOOL_CALENDAR_EVENT_LIST};

const EVENT_MODEL: &str = "calendar.event";

const EVENT_FIELDS: [&str; 7] = [
    "id",
    "name",
    "start",
    "stop",
    "location",
    "partner_ids",
    "description",
];

/// Hours an event runs for when the caller gives neither `stop` nor a duration.
pub const DEFAULT_DURATION_HOURS: f64 = 1.0;

// Why: a description is free text and can be an entire email thread. The
// briefing needs enough to recognise the event, not to read it.
const DESCRIPTION_CHARS: usize = 160;

/// Rewrite a caller's datetime into the form Odoo's ORM accepts.
///
/// Accepts `YYYY-MM-DDTHH:MM:SS`, a trailing `Z`, and fractional seconds, all
/// of which Odoo rejects verbatim. A value that is already in Odoo's own form
/// passes through untouched.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// each accepted shape; not part of the public API.
#[doc(hidden)]
#[must_use]
pub fn normalize_datetime(value: &str) -> String {
    let trimmed = value.trim().trim_end_matches('Z');
    let no_frac = trimmed.split_once('.').map_or(trimmed, |(head, _)| head);
    no_frac.replacen('T', " ", 1)
}

/// The event search domain: a start-date window and a name filter.
#[doc(hidden)]
#[must_use]
pub fn event_domain(input: &CalendarEventListInput) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = Vec::new();
    if let Some(from) = input.date_from.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        domain.push(serde_json::json!(["start", ">=", format!("{from} 00:00:00")]));
    }
    if let Some(to) = input.date_to.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        domain.push(serde_json::json!(["start", "<=", format!("{to} 23:59:59")]));
    }
    if let Some(query) = input.query.as_deref().map(str::trim).filter(|q| !q.is_empty()) {
        domain.push(serde_json::json!(["name", "ilike", format!("%{query}%")]));
    }
    serde_json::Value::Array(domain)
}

/// One event as a markdown row.
#[doc(hidden)]
#[must_use]
pub fn event_row(record: &serde_json::Value) -> String {
    let id = record
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    let attendees = record
        .get("partner_ids")
        .and_then(serde_json::Value::as_array)
        .map_or(0, Vec::len);
    let mut row = format!(
        "- **[{id}] {}** — {} → {} · {} · {attendees} attendee(s)",
        field_or_dash(record, "name"),
        field_or_dash(record, "start"),
        field_or_dash(record, "stop"),
        field_or_dash(record, "location"),
    );
    if let Some(description) = crate::format::field(record, "description") {
        let text = html_to_text(&description);
        if !text.is_empty() {
            let clipped: String = text.chars().take(DESCRIPTION_CHARS).collect();
            let ellipsis = if text.chars().count() > DESCRIPTION_CHARS {
                "…"
            } else {
                ""
            };
            row.push_str(&format!("\n  {clipped}{ellipsis}"));
        }
    }
    row
}

/// Build the `calendar.event` create payload.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the derived `stop` and the optional record link; not part of the public API.
#[doc(hidden)]
#[must_use]
pub fn event_values(input: &CalendarEventCreateInput) -> serde_json::Value {
    let start = normalize_datetime(&input.start);
    let mut values = serde_json::Map::new();
    values.insert("name".to_owned(), serde_json::json!(input.name.trim()));
    values.insert("start".to_owned(), serde_json::json!(start));
    values.insert("stop".to_owned(), serde_json::json!(stop_for(input, &start)));

    if let Some(partners) = input.attendee_partner_ids.as_ref().filter(|p| !p.is_empty()) {
        // Why: Odoo's x2many write format. `[(6, 0, ids)]` means "replace the
        // set with exactly these", which on a new record is simply "invite
        // them".
        values.insert(
            "partner_ids".to_owned(),
            serde_json::json!([[6, 0, partners]]),
        );
    }
    if let Some(description) = input.description.as_deref().map(str::trim).filter(|d| !d.is_empty())
    {
        values.insert("description".to_owned(), serde_json::json!(description));
    }
    // Why: both halves or neither. A res_id without its model is an id Odoo
    // cannot resolve, and it would silently attach the event to nothing.
    if let (Some(model), Some(res_id)) = (input.model.as_deref(), input.res_id) {
        values.insert("res_model".to_owned(), serde_json::json!(model));
        values.insert("res_id".to_owned(), serde_json::json!(res_id));
    }
    serde_json::Value::Object(values)
}

// Why: an explicit stop wins; otherwise the duration decides; otherwise one
// hour. Odoo requires a stop, so there is no "leave it open" option to pass on.
fn stop_for(input: &CalendarEventCreateInput, start: &str) -> String {
    if let Some(stop) = input.stop.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        return normalize_datetime(stop);
    }
    let hours = input.duration_hours.unwrap_or(DEFAULT_DURATION_HOURS);
    add_hours(start, hours).unwrap_or_else(|| start.to_owned())
}

// Why: Odoo's datetime format is fixed-width, so parsing it with chrono and
// adding the offset is exact. An unparseable start falls back to a zero-length
// event rather than refusing — Odoo will reject a malformed start anyway, and
// its error names the field.
fn add_hours(start: &str, hours: f64) -> Option<String> {
    let parsed = chrono::NaiveDateTime::parse_from_str(start, "%Y-%m-%d %H:%M:%S").ok()?;
    let minutes = (hours * 60.0).round();
    let delta = chrono::Duration::try_minutes(minutes as i64)?;
    Some(
        parsed
            .checked_add_signed(delta)?
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    )
}

#[derive(Debug)]
pub struct CalendarEventListHandler {
    pub call: OdooCall,
}

impl McpToolHandler for CalendarEventListHandler {
    type Input = CalendarEventListInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_CALENDAR_EVENT_LIST
    }

    fn description(&self) -> &'static str {
        "List calendar events in a date window."
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
                fields: EVENT_FIELDS.iter().map(|f| (*f).to_owned()).collect(),
                limit: resolve_limit(input.limit),
                order: Some("start asc".to_owned()),
            };
            let records = call
                .client
                .search_read(&call.creds, EVENT_MODEL, event_domain(&input), &options)
                .await?;

            let summary = format!("{} calendar event(s)", records.len());
            let body = if records.is_empty() {
                empty_result("calendar events")
            } else {
                records.iter().map(event_row).collect::<Vec<_>>().join("\n")
            };
            Ok((text_artifact("Odoo Calendar", &body), summary))
        }
    }
}

#[derive(Debug)]
pub struct CalendarEventCreateHandler {
    pub call: OdooCall,
}

impl McpToolHandler for CalendarEventCreateHandler {
    type Input = CalendarEventCreateInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_CALENDAR_EVENT_CREATE
    }

    fn description(&self) -> &'static str {
        "Create a calendar event, optionally linked to a record."
    }

    fn handle(
        &self,
        input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            if input.name.trim().is_empty() {
                return Err(McpError::invalid_params(
                    "An event title is required.".to_owned(),
                    None,
                ));
            }
            if input.model.is_some() != input.res_id.is_some() {
                return Err(McpError::invalid_params(
                    "To link the event to a record, give both model and res_id.".to_owned(),
                    None,
                ));
            }

            let values = event_values(&input);
            let id = call
                .client
                .create(&call.creds, EVENT_MODEL, values)
                .await?;

            let summary = format!(
                "Created calendar event {id} \"{}\" starting {}, organised by {}",
                input.name.trim(),
                normalize_datetime(&input.start),
                call.creds.login
            );
            Ok((text_artifact("Calendar Event Created", &summary), summary))
        }
    }
}
