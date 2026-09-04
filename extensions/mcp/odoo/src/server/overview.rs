//! `business_overview_data` — the daily briefing, in one call.
//!
//! Six queries fan out concurrently and come back as one dashboard: pipeline by
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

use super::briefing::fetch;
use super::call::OdooCall;
use super::overview_shape::briefing_dashboard;
use crate::tools::TOOL_OVERVIEW;
use crate::tools::inputs::OverviewInput;

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
            let dashboard = briefing_dashboard(&briefing).map_err(|e| {
                McpError::internal_error(format!("briefing did not serialise: {e}"), None)
            })?;
            Ok((CliArtifact::dashboard(dashboard), summary))
        }
    }
}
