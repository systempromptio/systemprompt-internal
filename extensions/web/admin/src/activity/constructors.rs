//! Shared entry point for the activity constructors.

use systemprompt_web_shared::format::truncate_chars;

pub(super) fn truncate(s: &str, max: usize) -> String {
    let head = truncate_chars(s, max);
    if head.len() == s.len() {
        return s.to_owned();
    }
    // Why: a description is prose, so the cut lands on the last word boundary
    // inside the budget rather than mid-word.
    let head = head.rfind(' ').map_or(head, |pos| &head[..pos]);
    format!("{head}\u{2026}")
}
