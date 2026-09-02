//! The row shapes the knowledge bank's queries return.
//!
//! Each is deliberately narrower than the table: a search hit carries a
//! snippet rather than the document, and a listing row carries a character
//! count rather than the text it counted. Nothing here returns `content` in
//! full — that is what search is for. The exception is [`ProposalDocument`],
//! which the projection pipeline needs whole.

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::KnowledgeBankError;
use crate::proposal::apply::AppliedOutcome;
use crate::proposal::intent::StructuredSummary;
use crate::proposal::{DocumentStatus, Proposal};

/// One search result: enough provenance to judge the hit, plus a snippet, but
/// never the whole document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub id: Uuid,
    pub title: String,
    pub source: String,
    pub project: Option<String>,
    pub created_at: DateTime<Utc>,
    pub uploaded_by: String,
    pub snippet: String,
}

/// One listing row. Carries `size` — the document's character count — instead
/// of its content, so a caller can tell a one-line note from a transcript
/// without paying for either.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSummary {
    pub id: Uuid,
    pub title: String,
    pub source: String,
    pub project: Option<String>,
    pub created_at: DateTime<Utc>,
    pub size: i32,
    pub status: String,
    pub category: Option<String>,
    pub summary: Option<String>,
}

/// What a successful upload tells the caller: the identity of the row and when
/// it landed.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct UploadedDocument {
    pub id: Uuid,
    pub created_at: DateTime<Utc>,
}


/// The fields an upload contributes, grouped rather than passed positionally.
///
/// Four of the five are strings, so a positional signature would let a caller
/// swap `source` and `uploaded_by` and get a clean compile with forged
/// provenance.
#[derive(Debug, Clone, Copy)]
pub struct NewDocument<'a> {
    pub title: &'a str,
    pub source: &'a str,
    pub project: Option<&'a str>,
    pub content: &'a str,
    pub uploaded_by: &'a str,
}

/// The typed `metadata` the email ingestion job writes on a document.
#[derive(Debug, Clone, Default, Serialize, Deserialize, JsonSchema)]
pub struct EmailMetadata {
    #[serde(rename = "message_id", default)]
    pub rfc5322_id: Option<String>,
    #[serde(default)]
    pub from: Option<String>,
    #[serde(default)]
    pub to: Option<String>,
    #[serde(default)]
    pub date: Option<String>,
    #[serde(default)]
    pub attachments: Vec<String>,
}

// JSON: SQL boundary — the JSONB columns arrive untyped and are typed by
// `ProposalDocument::try_from` before anything reads them.
#[derive(Debug)]
pub struct ProposalDocumentRow {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub category: Option<String>,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub structured: Option<serde_json::Value>,
    pub content: String,
    pub proposal: Option<serde_json::Value>,
    pub proposal_revision: i32,
    pub proposal_call_id: Option<String>,
    pub proposal_error: Option<String>,
    pub skip_reason: Option<String>,
    pub apply_attempts: i32,
    pub applied: Option<serde_json::Value>,
    pub decided_by: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
}

// JSON: SQL boundary — the feed shape, without `content`.
#[derive(Debug)]
pub struct FeedDocumentRow {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub category: Option<String>,
    pub status: String,
    pub metadata: Option<serde_json::Value>,
    pub structured: Option<serde_json::Value>,
    pub proposal: Option<serde_json::Value>,
    pub proposal_revision: i32,
    pub proposal_call_id: Option<String>,
    pub proposal_error: Option<String>,
    pub skip_reason: Option<String>,
    pub apply_attempts: i32,
    pub applied: Option<serde_json::Value>,
    pub decided_by: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
}

#[derive(Debug)]
pub struct SettleableRow {
    pub document_id: Uuid,
    pub call_id: String,
}

/// One email document with every pipeline column typed.
#[derive(Debug, Clone)]
pub struct ProposalDocument {
    pub id: Uuid,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub category: Option<String>,
    pub status: DocumentStatus,
    pub metadata: EmailMetadata,
    pub structured: Option<StructuredSummary>,
    pub content: String,
    pub proposal: Option<Proposal>,
    pub proposal_revision: i32,
    pub proposal_call_id: Option<String>,
    pub proposal_error: Option<String>,
    pub skip_reason: Option<String>,
    pub apply_attempts: i32,
    pub applied: Option<AppliedOutcome>,
    pub decided_by: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
}

impl ProposalDocument {
    // Why: a document ingested before the metadata column existed still needs
    // a stable Odoo-side idempotency key; its own id is the fallback.
    #[must_use]
    pub fn rfc5322_id(&self) -> String {
        self.metadata
            .rfc5322_id
            .clone()
            .unwrap_or_else(|| format!("<knowledge-{}@systemprompt.io>", self.id))
    }

    #[must_use]
    pub fn received(&self) -> Option<String> {
        self.metadata.date.clone()
    }
}

fn typed<T: serde::de::DeserializeOwned>(
    column: &str,
    value: Option<serde_json::Value>,
) -> Result<Option<T>, KnowledgeBankError> {
    value
        .map(serde_json::from_value)
        .transpose()
        .map_err(|e| KnowledgeBankError::Internal(format!("{column} column is not typed: {e}")))
}

fn status(value: &str) -> Result<DocumentStatus, KnowledgeBankError> {
    DocumentStatus::parse(value)
        .ok_or_else(|| KnowledgeBankError::Internal(format!("unknown document status {value}")))
}

impl TryFrom<ProposalDocumentRow> for ProposalDocument {
    type Error = KnowledgeBankError;

    fn try_from(row: ProposalDocumentRow) -> Result<Self, Self::Error> {
        Ok(Self {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            category: row.category,
            status: status(&row.status)?,
            metadata: typed("metadata", row.metadata)?.unwrap_or_default(),
            structured: typed("structured", row.structured)?,
            content: row.content,
            proposal: typed("proposal", row.proposal)?,
            proposal_revision: row.proposal_revision,
            proposal_call_id: row.proposal_call_id,
            proposal_error: row.proposal_error,
            skip_reason: row.skip_reason,
            apply_attempts: row.apply_attempts,
            applied: typed("applied", row.applied)?,
            decided_by: row.decided_by,
            decided_at: row.decided_at,
        })
    }
}

impl TryFrom<FeedDocumentRow> for ProposalDocument {
    type Error = KnowledgeBankError;

    fn try_from(row: FeedDocumentRow) -> Result<Self, Self::Error> {
        Self::try_from(ProposalDocumentRow {
            id: row.id,
            title: row.title,
            created_at: row.created_at,
            category: row.category,
            status: row.status,
            metadata: row.metadata,
            structured: row.structured,
            content: String::new(),
            proposal: row.proposal,
            proposal_revision: row.proposal_revision,
            proposal_call_id: row.proposal_call_id,
            proposal_error: row.proposal_error,
            skip_reason: row.skip_reason,
            apply_attempts: row.apply_attempts,
            applied: row.applied,
            decided_by: row.decided_by,
            decided_at: row.decided_at,
        })
    }
}
