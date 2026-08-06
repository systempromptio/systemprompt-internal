//! The row shapes the knowledge bank's queries return.
//!
//! Each is deliberately narrower than the table: a search hit carries a
//! snippet rather than the document, and a listing row carries a character
//! count rather than the text it counted. Nothing here returns `content` in
//! full — that is what search is for.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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
