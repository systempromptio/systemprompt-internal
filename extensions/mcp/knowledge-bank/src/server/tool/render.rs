//! How search hits and listing rows become the text the model reads.
//!
//! Both renderers return an explicit sentence when they have nothing to show:
//! an empty body reads as a broken tool rather than an answer. Both carry the
//! row's id, because the id is what a follow-up call needs and an agent that
//! has to guess it will guess wrong.

use crate::store::{DocumentSummary, SearchHit};

/// Shown when a query matched nothing, in place of an empty body — an empty
/// string reads as a broken tool rather than an answer.
pub const NO_MATCHES: &str = "No matching documents in the knowledge bank.";

/// Shown when a filtered listing is empty.
pub const NO_DOCUMENTS: &str = "The knowledge bank holds no documents matching the filter.";

pub(super) fn project_label(project: Option<&str>) -> &str {
    project.unwrap_or("unscoped")
}

/// Render search hits as the markdown body returned to the model.
///
/// Each hit leads with its title and provenance and carries the id, because
/// the id is what a follow-up call needs and an agent that has to guess it
/// will guess wrong.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the empty-result sentinel and the per-hit heading shape directly; not part
/// of the public API.
#[doc(hidden)]
#[must_use]
pub fn search_summary(hits: &[SearchHit]) -> String {
    if hits.is_empty() {
        return NO_MATCHES.to_owned();
    }
    hits.iter()
        .map(|h| {
            format!(
                "## {} ({}, {}, {})\n\nid: {}\nuploaded by: {}\n\n{}",
                h.title,
                h.source,
                project_label(h.project.as_deref()),
                h.created_at.format("%Y-%m-%d"),
                h.id,
                h.uploaded_by,
                h.snippet
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Render a listing as one line per document: identity, provenance, and size,
/// never content.
///
/// Exposed (behind `#[doc(hidden)]`) for the external test workspace; not part
/// of the public API.
#[doc(hidden)]
#[must_use]
pub fn listing_summary(documents: &[DocumentSummary]) -> String {
    if documents.is_empty() {
        return NO_DOCUMENTS.to_owned();
    }
    documents
        .iter()
        .map(|d| {
            format!(
                "- {} — {} ({}, {}, {}, {} chars)",
                d.id,
                d.title,
                d.source,
                project_label(d.project.as_deref()),
                d.created_at.format("%Y-%m-%d"),
                d.size
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}
