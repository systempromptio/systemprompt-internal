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

pub const MAX_CONTENT_BYTES: usize = 2 * 1024 * 1024;

pub const DEFAULT_SEARCH_LIMIT: u32 = 10;

pub const MAX_SEARCH_LIMIT: u32 = 50;

pub const MAX_LIST_LIMIT: i64 = 200;


/// Which query shape a caller's search string earns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    Newest,
    FullText,
}

#[must_use]
pub fn search_mode(query: &str) -> SearchMode {
    if query.trim().chars().count() < 2 {
        SearchMode::Newest
    } else {
        SearchMode::FullText
    }
}

#[must_use]
pub fn clamp_search_limit(limit: Option<u32>) -> i64 {
    i64::from(
        limit
            .unwrap_or(DEFAULT_SEARCH_LIMIT)
            .clamp(1, MAX_SEARCH_LIMIT),
    )
}

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

#[must_use]
pub fn normalize_optional(value: Option<String>) -> Option<String> {
    value.map(|v| v.trim().to_owned()).filter(|v| !v.is_empty())
}

pub fn require_non_empty(field: &str, value: &str) -> Result<String, KnowledgeBankError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(KnowledgeBankError::Invalid(format!(
            "{field} must not be empty"
        )));
    }
    Ok(trimmed.to_owned())
}
