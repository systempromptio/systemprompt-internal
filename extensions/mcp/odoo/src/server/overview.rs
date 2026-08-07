//! `business_overview_data` — the daily briefing, in one call.
//!
//! Six queries fan out concurrently and come back as one document: pipeline by
//! stage, leads created in the last seven days, the acting user's overdue and
//! due-today activities, the twenty most recent chatter notes, today's calendar
//! events, and open tasks falling due within the week. The composite exists
//! because the alternative — a model issuing six tool calls and stitching the
//! answers — costs six round trips and reliably forgets one.
//!
//! Every section is a bounded read. The point of a briefing is to be cheap
//! enough to run first thing, so nothing here scans a whole table.

use rmcp::ErrorData as McpError;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::mcp::McpToolHandler;
use systemprompt::models::artifacts::CliArtifact;
use systemprompt::models::execution::context::RequestContext;

use super::activity::activity_row;
use super::briefing::{Briefing, RECENT_DAYS, TASK_HORIZON_DAYS, fetch};
use super::calendar::event_row;
use super::call::OdooCall;
use super::crm::lead_row;
use super::report::group_row;
use super::tasks::task_row;
use crate::format::{field_or_dash, text_artifact};
use crate::tools::TOOL_OVERVIEW;
use crate::tools::inputs::OverviewInput;

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
                 event(s) today, {} task(s) due this week, {} recent note(s)",
                call.creds.login,
                briefing.pipeline.len(),
                briefing.new_leads.len(),
                briefing.activities.len(),
                briefing.events.len(),
                briefing.tasks.len(),
                briefing.notes.len()
            );
            Ok((
                text_artifact("Odoo Business Overview", &render(&briefing)),
                summary,
            ))
        }
    }
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
            "Today's calendar",
            &briefing.events.iter().map(event_row).collect::<Vec<_>>(),
            "Nothing in the calendar today.",
        ),
        section(
            &format!("Open tasks due in the next {TASK_HORIZON_DAYS} days"),
            &briefing.tasks.iter().map(task_row).collect::<Vec<_>>(),
            "No tasks fall due this week.",
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
