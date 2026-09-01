//! The typed payloads the `proposal_*` tools return.
//!
//! Dashboards consume these as `structuredContent`; the text body is the same
//! JSON, so a client that did not negotiate structured content still gets a
//! machine-readable answer after the one-line summary.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt::mcp::McpOutputSchema;

use crate::proposal::apply::AppliedOutcome;
use crate::proposal::{DocumentStatus, Proposal};
use crate::store::ProposalDocument;

/// Whether the caller can apply a proposal, and how to fix it if not.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ViewerCapability {
    pub can_apply: bool,
    pub odoo_login: Option<String>,
    pub link_url: String,
}

/// One feed entry: the document with its pipeline state and proposal.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct FeedRow {
    pub id: String,
    pub title: String,
    pub sender: Option<String>,
    pub received: Option<String>,
    pub created_at: String,
    pub category: Option<String>,
    pub status: DocumentStatus,
    pub summary: Option<String>,
    pub proposal: Option<Proposal>,
    pub applied: Option<AppliedOutcome>,
    pub proposal_error: Option<String>,
    pub skip_reason: Option<String>,
    pub decided_by: Option<String>,
    pub decided_at: Option<String>,
    pub attempts: i32,
}

impl From<&ProposalDocument> for FeedRow {
    fn from(doc: &ProposalDocument) -> Self {
        Self {
            id: doc.id.to_string(),
            title: doc.title.clone(),
            sender: doc.metadata.from.clone(),
            received: doc.metadata.date.clone(),
            created_at: doc.created_at.to_rfc3339(),
            category: doc.category.clone(),
            status: doc.status,
            summary: doc.structured.as_ref().map(|s| s.summary.clone()),
            proposal: doc.proposal.clone(),
            applied: doc.applied.clone(),
            proposal_error: doc.proposal_error.clone(),
            skip_reason: doc.skip_reason.clone(),
            decided_by: doc.decided_by.clone(),
            decided_at: doc.decided_at.map(|d| d.to_rfc3339()),
            attempts: doc.apply_attempts,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalListOutput {
    pub viewer: ViewerCapability,
    pub odoo_web_base: Option<String>,
    pub rows: Vec<FeedRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalGetOutput {
    pub row: FeedRow,
    pub body_html: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProposalDecideOutput {
    pub document_id: String,
    pub status: DocumentStatus,
    pub applied: Option<AppliedOutcome>,
    pub message: String,
}

// Why: the text body is the JSON itself, so a client that does not negotiate
// structuredContent still receives the same machine-readable payload.
macro_rules! json_output {
    ($ty:ty, $name:expr) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }

            fn text_body(&self) -> Option<String> {
                serde_json::to_string(self).ok()
            }
        }
    };
}
json_output!(ProposalListOutput, "knowledge_proposal_list");
json_output!(ProposalGetOutput, "knowledge_proposal");
json_output!(ProposalDecideOutput, "knowledge_proposal_decision");
