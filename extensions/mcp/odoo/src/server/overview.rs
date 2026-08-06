//! `business_overview_data` — the daily briefing, in one call.
//!
//! Four queries fan out concurrently and come back as one document: pipeline
//! by stage, leads created in the last seven days, the acting user's overdue
//! and due-today activities, and the twenty most recent chatter notes. The
//! composite exists because the alternative — a model issuing four tool calls
//! and stitching the answers — costs four round trips and reliably forgets one.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::call::{OdooCall, lead_fields};
use super::crm::lead_row;
use super::activity::{activity_fields, activity_row};
use super::report::group_row;
use crate::client::{GroupQuery, SearchOptions};
use crate::format::{field_or_dash, text_artifact};
use crate::tools::TOOL_OVERVIEW;
use crate::tools::inputs::OverviewInput;

// Why: how far back "recent" reaches for the new-leads section.
const RECENT_DAYS: i64 = 7;
// Why: chatter notes included in the briefing.
const RECENT_NOTES: u32 = 20;
// Why: caps the new-leads list, so a busy week cannot crowd out the rest.
const RECENT_LEADS: u32 = 25;
// Why: chatter rows carry the record they hang off, so the briefing can say
// which lead a note was about without a second query.
const NOTE_FIELDS: [&str; 7] = [
    "id",
    "subject",
    "record_name",
    "model",
    "res_id",
    "author_id",
    "date",
];

fn section(title: &str, rows: &[String], empty: &str) -> String {
    let body = if rows.is_empty() {
        empty.to_owned()
    } else {
        rows.join("\n")
    };
    format!("## {title}\n\n{body}")
}

#[derive(Debug)]
pub struct OverviewHandler {
    pub call: OdooCall,
}

impl McpToolHandler for OverviewHandler {
    type Input = OverviewInput;
    type Output = CliArtifact;

    fn tool_name(&self) -> &'static str {
        TOOL_OVERVIEW
    }

    fn description(&self) -> &'static str {
        "Aggregate today's CRM picture from Odoo in a single call."
    }

    fn handle(
        &self,
        _input: Self::Input,
        _ctx: &RequestContext,
        _exec_id: &McpExecutionId,
    ) -> impl Future<Output = Result<(Self::Output, String), McpError>> + Send {
        let call = self.call.clone();
        async move {
            let briefing = fetch(&call).await?;
            let summary = format!(
                "Odoo overview for {}: {} stage(s), {} new lead(s), {} activity(ies) due, {} \
                 recent note(s)",
                call.creds.login,
                briefing.pipeline.len(),
                briefing.new_leads.len(),
                briefing.activities.len(),
                briefing.notes.len()
            );
            Ok((
                text_artifact("Odoo Business Overview", &render(&briefing)),
                summary,
            ))
        }
    }
}

// Why: the four result sets a briefing is assembled from, kept together so
// the fan-out and the rendering agree on what a briefing contains.
#[derive(Debug)]
struct Briefing {
    pipeline: Vec<serde_json::Value>,
    new_leads: Vec<serde_json::Value>,
    activities: Vec<serde_json::Value>,
    notes: Vec<serde_json::Value>,
}

// Why: the four queries are independent, so they go out together — a briefing
// that cost four sequential round trips to Odoo would be the slowest tool on
// the server, and it is the one most likely to be called first thing.
async fn fetch(call: &OdooCall) -> Result<Briefing, McpError> {
    let now = chrono::Utc::now();
    let today = now.date_naive().to_string();
    let since = (now - chrono::Duration::days(RECENT_DAYS))
        .date_naive()
        .to_string();

    let lead_options = SearchOptions {
        fields: lead_fields(),
        limit: RECENT_LEADS,
        order: Some("create_date desc".to_owned()),
    };
    let activity_options = SearchOptions {
        fields: activity_fields(),
        limit: RECENT_NOTES,
        order: Some("date_deadline asc".to_owned()),
    };
    let note_options = SearchOptions {
        fields: NOTE_FIELDS.iter().map(|f| (*f).to_owned()).collect(),
        limit: RECENT_NOTES,
        order: Some("date desc".to_owned()),
    };

    let pipeline = call.client.read_group(&call.creds, "crm.lead", GroupQuery {
        domain: serde_json::json!([]),
        fields: &["expected_revenue:sum"],
        group_by: &["stage_id"],
    });
    let new_leads = call.client.search_read(
        &call.creds,
        "crm.lead",
        serde_json::json!([["create_date", ">=", since]]),
        &lead_options,
    );
    // Why: `<=` today, not `<` — the briefing is for planning the day, so what
    // is due today belongs beside what is already late.
    let activities = call.client.search_read(
        &call.creds,
        "mail.activity",
        serde_json::json!([["user_id", "=", call.creds.uid], ["date_deadline", "<=", today]]),
        &activity_options,
    );
    let notes = call.client.search_read(
        &call.creds,
        "mail.message",
        serde_json::json!([["message_type", "=", "comment"]]),
        &note_options,
    );

    let (pipeline, new_leads, activities, notes) =
        tokio::try_join!(pipeline, new_leads, activities, notes)?;
    Ok(Briefing {
        pipeline,
        new_leads,
        activities,
        notes,
    })
}

// Why: renders the briefing as one markdown document. Each section names its
// own empty case, so a quiet day reads as a quiet day rather than as a broken
// query.
fn render(briefing: &Briefing) -> String {
    let sections = [
        section(
            "Pipeline by stage",
            &briefing
                .pipeline
                .iter()
                .map(|r| group_row(r, "stage_id"))
                .collect::<Vec<_>>(),
            "No leads in the pipeline.",
        ),
        section(
            &format!("Leads created in the last {RECENT_DAYS} days"),
            &briefing.new_leads.iter().map(lead_row).collect::<Vec<_>>(),
            "No new leads this week.",
        ),
        section(
            "Your activities, overdue and due today",
            &briefing
                .activities
                .iter()
                .map(activity_row)
                .collect::<Vec<_>>(),
            "Nothing due — your activity list is clear.",
        ),
        section(
            "Recent notes",
            &briefing.notes.iter().map(note_row).collect::<Vec<_>>(),
            "No recent chatter.",
        ),
    ];
    sections.join("\n\n")
}

// Why: the briefing names each note's model and id, not just the record's
// display name. A reader who wants the rest of the thread needs an anchor they
// can pass straight to note_list; a display name alone is not one.
fn note_row(record: &serde_json::Value) -> String {
    format!(
        "- {} on {} {} ({}) by {} — {}",
        field_or_dash(record, "date"),
        field_or_dash(record, "model"),
        field_or_dash(record, "res_id"),
        field_or_dash(record, "record_name"),
        field_or_dash(record, "author_id"),
        field_or_dash(record, "subject")
    )
}
