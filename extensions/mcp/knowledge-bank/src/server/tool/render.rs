//! How search hits and listing rows become the text the model reads.
//!
//! Both renderers return an explicit sentence when they have nothing to show:
//! an empty body reads as a broken tool rather than an answer. Both carry the
//! row's id, because the id is what a follow-up call needs and an agent that
//! has to guess it will guess wrong.

use crate::store::{DocumentSummary, SearchHit};

pub const NO_MATCHES: &str = "No matching documents in the knowledge bank.";

pub const NO_DOCUMENTS: &str = "The knowledge bank holds no documents matching the filter.";

pub(super) fn project_label(project: Option<&str>) -> &str {
    project.unwrap_or("unscoped")
}

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

#[doc(hidden)]
#[must_use]
pub fn listing_summary(documents: &[DocumentSummary]) -> String {
    if documents.is_empty() {
        return NO_DOCUMENTS.to_owned();
    }
    documents
        .iter()
        .map(|d| {
            let mut line = format!(
                "- {} — {} ({}, {}, {}, {} chars, {}",
                d.id,
                d.title,
                d.source,
                project_label(d.project.as_deref()),
                d.created_at.format("%Y-%m-%d"),
                d.size,
                d.status
            );
            if let Some(category) = d.category.as_deref() {
                line.push_str(", ");
                line.push_str(category);
            }
            line.push(')');
            if let Some(summary) = d.summary.as_deref() {
                line.push_str("\n  ");
                line.push_str(summary);
            }
            line
        })
        .collect::<Vec<_>>()
        .join("\n")
}
