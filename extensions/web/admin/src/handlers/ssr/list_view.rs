//! The pieces every filtered entity-list page renders the same way.
//!
//! Every filtered list page binds to the same URL contract —
//! `?preset=&from=&to=&<facet>=&page=` — and feeds the same
//! `components/time-range`, `components/identity-filter-ribbon` and pagination
//! partials. The types below are the serialisation those partials read, and
//! the functions build them from a page's own query parameters expressed as a
//! `(name, value)` slice, so a list page supplies its parameter list and
//! inherits the behaviour rather than copying it.

use serde::Serialize;
use urlencoding::encode as urlencode;

use crate::repositories::governance::filter_options::FilterOption;
use crate::util::time_range::{TimeRange, TimeRangePreset, TimeRangeQuery};

// Why: A page's query parameters, in the order they should appear in a rebuilt
// URL.
pub(crate) type QueryPairs<'a> = [(&'a str, Option<&'a str>)];

#[derive(Debug, Serialize)]
pub(crate) struct TimeRangeContext {
    pub(crate) preset: String,
    pub(crate) from: String,
    pub(crate) to: String,
    pub(crate) base_url: &'static str,
    pub(crate) query: &'static str,
}

#[derive(Debug, Serialize)]
pub(crate) struct Preserved {
    pub(crate) name: &'static str,
    pub(crate) value: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct AnnotatedOption {
    pub(crate) id: String,
    pub(crate) label: String,
    pub(crate) count: i64,
    pub(crate) selected: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct Chip {
    pub(crate) group_label: &'static str,
    pub(crate) label: String,
    pub(crate) value: String,
    pub(crate) remove_url: String,
}

// Why: one bundle for the five numbers a footer needs, so each page's
// `build_pagination` takes a window rather than a row of bare integers.
#[derive(Debug, Clone, Copy)]
pub struct PageWindow {
    // Why: Zero-based page index.
    pub index: i64,
    pub size: i64,
    pub total_pages: i64,
    pub total_rows: i64,
    // Why: Rows actually rendered on this page — the last page is short.
    pub shown_rows: i64,
    // Why: What the rows are called: "Showing 1-50 of 54 <noun>".
    pub noun: &'static str,
}

impl PageWindow {
    // Why: an empty result still renders as "page 1 of 1" rather than "of 0",
    // which is the one case the ceiling division cannot produce on its own.
    pub const fn new(
        index: i64,
        size: i64,
        total_rows: i64,
        shown_rows: i64,
        noun: &'static str,
    ) -> Self {
        let total_pages = if total_rows == 0 {
            1
        } else {
            (total_rows + size - 1) / size
        };
        Self {
            index,
            size,
            total_pages,
            total_rows,
            shown_rows,
            noun,
        }
    }

    // Why: The 1-based inclusive row range this page covers, `(0, 0)` when empty.
    pub const fn bounds(self) -> (i64, i64) {
        if self.shown_rows == 0 {
            return (0, 0);
        }
        let first = self.index * self.size + 1;
        (first, self.index * self.size + self.shown_rows)
    }
}

#[derive(Debug, Serialize)]
pub(crate) struct Pagination {
    pub(crate) current_page: i64,
    pub(crate) total_pages: i64,
    // Why: 1-based row range for "Showing 1-50 of 54"; `first_row` is 0 only
    // when the page is empty.
    pub(crate) first_row: i64,
    pub(crate) last_row: i64,
    pub(crate) total_rows: i64,
    pub(crate) noun: &'static str,
    pub(crate) has_prev: bool,
    pub(crate) has_next: bool,
    pub(crate) prev_url: Option<String>,
    pub(crate) next_url: Option<String>,
}

// Why: Drops empty values so a rebuilt URL never carries `&user_id=`.
pub(crate) fn empty_to_none(v: Option<&str>) -> Option<&str> {
    v.filter(|s| !s.is_empty())
}

// Why: The preset name to echo back into the URL.
//
// An explicit `?preset=` wins, then an explicit `from`+`to` pair means
// `custom`; otherwise the parsed range's own preset is authoritative.
pub(crate) fn preset_str(query: &TimeRangeQuery, range: TimeRange) -> String {
    if let Some(p) = empty_to_none(query.preset.as_deref()) {
        return p.to_owned();
    }
    if query.from.is_some() && query.to.is_some() {
        return "custom".to_owned();
    }
    match range.preset {
        TimeRangePreset::Min15 => "15m",
        TimeRangePreset::Hour1 => "1h",
        TimeRangePreset::Hours24 => "24h",
        TimeRangePreset::Days7 => "7d",
        TimeRangePreset::Days30 => "30d",
        TimeRangePreset::Custom => "custom",
    }
    .to_owned()
}

pub(crate) fn time_range_context(
    base_url: &'static str,
    range: TimeRange,
    preset: &str,
) -> TimeRangeContext {
    TimeRangeContext {
        preset: preset.to_owned(),
        from: range.from.to_rfc3339(),
        to: range.to.to_rfc3339(),
        base_url,
        query: "",
    }
}

// Why: Hidden inputs the filter-ribbon form must resubmit so that choosing a
// facet does not silently reset the time window.
pub(crate) fn build_preserved(
    range: TimeRange,
    preset: &str,
    extra: &[(&'static str, Option<&str>)],
) -> Vec<Preserved> {
    let mut out = vec![
        Preserved {
            name: "preset",
            value: preset.to_owned(),
        },
        Preserved {
            name: "from",
            value: range.from.to_rfc3339(),
        },
        Preserved {
            name: "to",
            value: range.to.to_rfc3339(),
        },
    ];
    out.extend(extra.iter().filter_map(|(name, value)| {
        empty_to_none(*value).map(|v| Preserved {
            name,
            value: v.to_owned(),
        })
    }));
    out
}

pub(crate) fn query_string(pairs: &QueryPairs<'_>, drop: &[&str]) -> String {
    pairs
        .iter()
        .filter(|(name, _)| !drop.contains(name))
        .filter_map(|(name, val)| empty_to_none(*val).map(|v| format!("{}={}", name, urlencode(v))))
        .collect::<Vec<_>>()
        .join("&")
}

fn url_without(base_url: &str, pairs: &QueryPairs<'_>, drop: &[&str]) -> String {
    let qs = query_string(pairs, drop);
    if qs.is_empty() {
        base_url.to_owned()
    } else {
        format!("{base_url}?{qs}")
    }
}

// Why: One removable chip per active facet, in the order the groups are listed.
pub(crate) fn build_chips(
    base_url: &str,
    pairs: &QueryPairs<'_>,
    groups: &[(&str, &'static str)],
) -> Vec<Chip> {
    groups
        .iter()
        .filter_map(|(param, group_label)| {
            let value = pairs
                .iter()
                .find(|(name, _)| name == param)
                .and_then(|(_, v)| empty_to_none(*v))?;
            Some(Chip {
                group_label,
                label: value.to_owned(),
                value: value.to_owned(),
                remove_url: url_without(base_url, pairs, &[param]),
            })
        })
        .collect()
}

pub(crate) fn annotate_group(
    items: &[FilterOption],
    selected: Option<&str>,
) -> Vec<AnnotatedOption> {
    items
        .iter()
        .map(|o| AnnotatedOption {
            id: o.id.clone(),
            label: o.label.clone(),
            count: o.count,
            selected: selected.is_some_and(|s| s == o.id),
        })
        .collect()
}

// Why: `page` is the zero-based index; the rendered `current_page` is 1-based.
pub(crate) fn build_pagination(
    base_url: &str,
    pairs: &QueryPairs<'_>,
    window: PageWindow,
) -> Pagination {
    let page = window.index;
    let qs = query_string(pairs, &["page"]);
    let prefix = if qs.is_empty() {
        format!("{base_url}?")
    } else {
        format!("{base_url}?{qs}&")
    };
    let prev_url = (page > 0).then(|| format!("{prefix}page={}", page - 1));
    let next_url = (page + 1 < window.total_pages).then(|| format!("{prefix}page={}", page + 1));
    let (first_row, last_row) = window.bounds();
    Pagination {
        current_page: page + 1,
        total_pages: window.total_pages,
        first_row,
        last_row,
        total_rows: window.total_rows,
        noun: window.noun,
        has_prev: prev_url.is_some(),
        has_next: next_url.is_some(),
        prev_url,
        next_url,
    }
}
