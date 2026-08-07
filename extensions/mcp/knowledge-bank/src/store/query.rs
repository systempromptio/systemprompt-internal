//! The query-shaping decisions the store makes before any SQL runs: the
//! limits it enforces, which search mode a query earns, how an `ILIKE`
//! pattern is escaped, and the gates an upload has to pass.
//!
//! These are pure functions on purpose. They carry the policy worth pinning —
//! that an oversized limit is clamped rather than refused, that a one-
//! character query lists rather than matches, that a caller's `%` is a
//! percent sign — and keeping them free of the pool means that policy can be
//! tested without one.

use crate::error::KnowledgeBankError;

/// Upload ceiling on `content`, in bytes of UTF-8.
///
/// Two megabytes is roughly a 300-page transcript: large enough that no real
/// meeting or document is refused, small enough that a runaway paste cannot
/// push a multi-hundred-megabyte row through the tsvector generator.
pub const MAX_CONTENT_BYTES: usize = 2 * 1024 * 1024;

/// Default `search_project_context` page size when the caller does not ask.
pub const DEFAULT_SEARCH_LIMIT: u32 = 10;

/// Hard ceiling on `search_project_context`, applied to whatever the caller
/// asks for rather than refusing an oversized request.
pub const MAX_SEARCH_LIMIT: u32 = 50;

/// Ceiling on `list_documents`. Listing is a browse affordance, not an export
/// path; a knowledge bank large enough to hit this wants search instead.
pub const MAX_LIST_LIMIT: i64 = 200;


/// Which query shape a caller's search string earns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Nothing to match on — list the newest documents instead.
    Newest,
    /// Rank with `websearch_to_tsquery`, falling back to `ILIKE` if the
    /// tokenizer produces no lexemes.
    FullText,
}

/// Decide the search mode from the raw query.
///
/// A single character is treated as empty: `ILIKE '%a%'` matches essentially
/// every document, which is a worse answer than "here is what is newest".
#[must_use]
pub fn search_mode(query: &str) -> SearchMode {
    if query.trim().chars().count() < 2 {
        SearchMode::Newest
    } else {
        SearchMode::FullText
    }
}

/// Clamp a caller-supplied search limit into `1..=MAX_SEARCH_LIMIT`.
///
/// Out-of-range values are clamped rather than refused: an agent asking for
/// 1000 results wants "as many as you have", not an error.
#[must_use]
pub fn clamp_search_limit(limit: Option<u32>) -> i64 {
    i64::from(
        limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, MAX_SEARCH_LIMIT),
    )
}

/// Build the `ILIKE` pattern for the no-lexeme fallback.
///
/// The wildcards Postgres would otherwise read out of the caller's text are
/// escaped, so a query containing `%` searches for a percent sign rather than
/// matching everything.
#[must_use]
pub fn like_pattern(query: &str) -> String {
    let mut escaped = String::with_capacity(query.len() + 2);
    escaped.push('%');
    for c in query.trim().chars() {
        if matches!(c, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(c);
    }
    escaped.push('%');
    escaped
}

/// Reject a document whose content exceeds [`MAX_CONTENT_BYTES`].
///
/// # Errors
/// [`KnowledgeBankError::TooLarge`] with both the actual and permitted size,
/// so the caller can decide whether to split the document or drop it.
pub fn check_content_size(content: &str) -> Result<(), KnowledgeBankError> {
    if content.len() > MAX_CONTENT_BYTES {
        return Err(KnowledgeBankError::TooLarge(format!(
            "document content is {} bytes; the knowledge bank accepts at most {MAX_CONTENT_BYTES} \
             bytes ({} MB). Split the document and upload it in parts.",
            content.len(),
            MAX_CONTENT_BYTES / (1024 * 1024)
        )));
    }
    Ok(())
}

/// Treat a blank string the same as an absent one.
///
/// The MCP wire has no way to distinguish "the caller omitted project" from
/// "the caller sent an empty string", and the two mean the same thing here.
#[must_use]
pub fn normalize_optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

/// Reject an upload field that is blank once trimmed.
///
/// # Errors
/// [`KnowledgeBankError::Invalid`] naming the field.
pub fn require_non_empty(field: &str, value: &str) -> Result<String, KnowledgeBankError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(KnowledgeBankError::Invalid(format!(
            "{field} must not be empty"
        )));
    }
    Ok(trimmed.to_owned())
}
