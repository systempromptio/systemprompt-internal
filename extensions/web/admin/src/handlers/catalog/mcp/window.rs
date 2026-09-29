//! The window the MCP pages answer for, and how it travels in a URL.
//!
//! One range serves the tiles, the table, the detail sections and the export
//! links, so the helpers that parse it and spell it back out live together:
//! a page that parsed `?preset=` one way and wrote it another would send the
//! export a different question from the one on screen.

use crate::export::view::query_string;
use crate::handlers::ssr::list_view::TimeRangeContext;
use crate::util::time_range::{TimeRange, TimeRangePreset, TimeRangeQuery, parse_time_range};

pub(super) fn window_of(
    preset: Option<String>,
    from: Option<String>,
    to: Option<String>,
) -> TimeRange {
    parse_time_range(&TimeRangeQuery { from, to, preset })
}

pub(super) fn window_label(range: &TimeRange) -> String {
    match range.preset {
        TimeRangePreset::Custom => format!(
            "{} to {}",
            range.from.format("%Y-%m-%d %H:%M"),
            range.to.format("%Y-%m-%d %H:%M")
        ),
        preset => format!("last {}", preset.label()),
    }
}

// Why: the `(name, value)` pairs that name a window in a URL, shared by the
// sort links, the pagination and the export so none of them can drop it.
pub(super) fn window_pairs(range: &TimeRange) -> Vec<(&'static str, Option<String>)> {
    match range.preset {
        TimeRangePreset::Custom => vec![
            ("preset", Some("custom".to_owned())),
            ("from", Some(range.from.to_rfc3339())),
            ("to", Some(range.to.to_rfc3339())),
        ],
        preset => vec![("preset", Some(preset.as_str().to_owned()))],
    }
}

pub(super) fn pairs_to_query(pairs: &[(&str, Option<String>)]) -> String {
    let borrowed: Vec<(&str, Option<&str>)> =
        pairs.iter().map(|(k, v)| (*k, v.as_deref())).collect();
    query_string(&borrowed)
}

pub(super) fn time_range_context(range: &TimeRange, base_url: &'static str) -> TimeRangeContext {
    TimeRangeContext {
        preset: range.preset.as_str().to_owned(),
        from: range.from.format("%Y-%m-%dT%H:%M").to_string(),
        to: range.to.format("%Y-%m-%dT%H:%M").to_string(),
        base_url,
        query: "",
        rejected: range.rejected_bounds,
    }
}
