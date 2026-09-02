//! The human reading of a `brain_email_ingest` approval row: who wrote in,
//! about what, and what the pipeline proposes to do with it.
//!
//! A read-only view of `knowledge-bank`'s `ProposalArguments`, typed here
//! rather than imported so the admin crate does not link the MCP server.
//! Every field is optional: a shape change upstream degrades to the raw JSON
//! block, which the page always shows anyway, rather than to an error page.

use serde::{Deserialize, Serialize};
use systemprompt::security::policy::ApprovalRequest;

pub(super) const INGESTION_RULE: &str = "brain_email_ingest";

#[derive(Debug, Serialize)]
pub(super) struct IngestSummary {
    document_id: String,
    sender: String,
    subject: String,
    actions: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct IngestArgumentsView {
    #[serde(default)]
    document_id: String,
    #[serde(default)]
    proposal: IngestProposalView,
}

#[derive(Debug, Default, Deserialize)]
struct IngestProposalView {
    #[serde(default)]
    sender: IngestSenderView,
    #[serde(default)]
    actions: Vec<IngestActionView>,
}

#[derive(Debug, Default, Deserialize)]
struct IngestSenderView {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    email: String,
}

#[derive(Debug, Deserialize)]
struct IngestActionView {
    #[serde(default)]
    kind: String,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    subject: Option<String>,
    #[serde(default)]
    summary: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    project: Option<String>,
    #[serde(default)]
    target: Option<IngestTargetView>,
}

#[derive(Debug, Deserialize)]
struct IngestTargetView {
    #[serde(default)]
    label: Option<String>,
}

impl IngestActionView {
    fn label(&self) -> String {
        let target = self
            .target
            .as_ref()
            .and_then(|t| t.label.clone())
            .unwrap_or_else(|| "the new lead".to_owned());
        match self.kind.as_str() {
            "create_lead" => format!(
                "Create lead \u{201c}{}\u{201d}",
                self.title.as_deref().unwrap_or("")
            ),
            "post_chatter" => format!("Log the email on {target}"),
            "create_activity" => format!(
                "Schedule \u{201c}{}\u{201d} on {target}",
                self.summary.as_deref().unwrap_or("")
            ),
            "create_task" => format!(
                "Create task \u{201c}{}\u{201d} in {}",
                self.name.as_deref().unwrap_or(""),
                self.project.as_deref().unwrap_or("")
            ),
            other => other.to_owned(),
        }
    }
}

pub(super) fn ingest_summary(request: &ApprovalRequest) -> Option<IngestSummary> {
    if request.rule != INGESTION_RULE {
        return None;
    }
    let view: IngestArgumentsView = serde_json::from_value(request.arguments.clone()).ok()?;
    let sender = match &view.proposal.sender.name {
        Some(name) => format!("{name} <{}>", view.proposal.sender.email),
        None => view.proposal.sender.email.clone(),
    };
    let subject = view
        .proposal
        .actions
        .iter()
        .find_map(|a| a.subject.clone().or_else(|| a.title.clone()))
        .unwrap_or_default();
    Some(IngestSummary {
        document_id: view.document_id,
        sender,
        subject,
        actions: view
            .proposal
            .actions
            .iter()
            .map(IngestActionView::label)
            .collect(),
    })
}

pub(super) fn humanize(seconds: i64) -> String {
    match seconds {
        s if s < 90 => format!("{s}s"),
        s if s < 90 * 60 => format!("{}m", s / 60),
        s if s < 36 * 3600 => format!("{}h", s / 3600),
        s => format!("{}d", s / 86400),
    }
}
