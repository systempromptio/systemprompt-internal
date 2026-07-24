//! View-model assembly for the Trace Explorer list page.
//!
//! Pure functions that turn repository rows + the parsed query into the typed
//! context the `perf-traces` template renders: filter ribbon, chips,
//! pagination, per-trace rows, and the display formatting they depend on.

use urlencoding::encode as urlencode;

use crate::repositories::governance::filter_options::{FilterOption, FilterOptions};
use crate::repositories::traces::{TraceFilter, TraceSortColumn, TraceSortDir, TraceStats};
use crate::util::time_range::TimeRange;

use super::context::{
    AnnotatedOption, Chip, Pagination, Preserved, SortHeader, SortHeaders, StatsView,
    TimeRangeContext, TraceFilterOptionsView,
};
use super::{BASE_URL, TraceListQuery, empty_to_none};

pub(super) const fn sort_col_to_str(c: TraceSortColumn) -> &'static str {
    match c {
        TraceSortColumn::StartedAt => "started_at",
        TraceSortColumn::Duration => "duration",
        TraceSortColumn::SpanCount => "spans",
        TraceSortColumn::Cost => "cost",
        TraceSortColumn::Tokens => "tokens",
    }
}

pub(super) const fn sort_dir_to_str(d: TraceSortDir) -> &'static str {
    match d {
        TraceSortDir::Asc => "asc",
        TraceSortDir::Desc => "desc",
    }
}

pub(super) fn time_range_context(range: TimeRange, preset: &str) -> TimeRangeContext {
    TimeRangeContext {
        preset: preset.to_owned(),
        from: range.from.to_rfc3339(),
        to: range.to.to_rfc3339(),
        base_url: BASE_URL,
        query: "",
    }
}

pub(super) fn build_preserved(
    query: &TraceListQuery,
    range: TimeRange,
    preset: &str,
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
    if query.error_only.as_deref() == Some("true") {
        out.push(Preserved {
            name: "error_only",
            value: "true".to_owned(),
        });
    }
    if query.deny_only.as_deref() == Some("true") {
        out.push(Preserved {
            name: "deny_only",
            value: "true".to_owned(),
        });
    }
    out
}

pub(super) fn build_chips(query: &TraceListQuery) -> Vec<Chip> {
    const GROUPS: &[(&str, &str)] = &[
        ("user_id", "User"),
        ("agent_id", "Agent"),
        ("agent_scope", "Scope"),
        ("policy", "Policy"),
        ("decision", "Decision"),
    ];
    let mut chips = Vec::new();
    for (param, label) in GROUPS {
        let val = match *param {
            "user_id" => query
                .user_id
                .as_ref()
                .map(systemprompt::identifiers::UserId::as_str),
            "agent_id" => query
                .agent_id
                .as_ref()
                .map(systemprompt::identifiers::AgentId::as_str),
            "agent_scope" => query.agent_scope.as_deref(),
            "policy" => query.policy.as_deref(),
            "decision" => query.decision.as_deref(),
            _ => None,
        };
        let Some(v) = empty_to_none(val) else {
            continue;
        };
        chips.push(Chip {
            group_label: label,
            label: v.to_owned(),
            value: v.to_owned(),
            remove_url: chip_remove_url(query, param),
        });
    }
    chips
}

fn chip_remove_url(query: &TraceListQuery, drop: &str) -> String {
    let qs = preserved_query_string(query, &[drop]);
    if qs.is_empty() {
        BASE_URL.to_owned()
    } else {
        format!("{BASE_URL}?{qs}")
    }
}

fn preserved_query_string(query: &TraceListQuery, drop: &[&str]) -> String {
    let pairs: [(&str, Option<&str>); 12] = [
        ("preset", query.preset.as_deref()),
        ("from", query.from.as_deref()),
        ("to", query.to.as_deref()),
        (
            "user_id",
            query
                .user_id
                .as_ref()
                .map(systemprompt::identifiers::UserId::as_str),
        ),
        (
            "agent_id",
            query
                .agent_id
                .as_ref()
                .map(systemprompt::identifiers::AgentId::as_str),
        ),
        ("agent_scope", query.agent_scope.as_deref()),
        ("policy", query.policy.as_deref()),
        ("decision", query.decision.as_deref()),
        ("error_only", query.error_only.as_deref()),
        ("deny_only", query.deny_only.as_deref()),
        ("sort", query.sort.as_deref()),
        ("dir", query.dir.as_deref()),
    ];
    pairs
        .iter()
        .filter(|(name, _)| !drop.contains(name))
        .filter_map(|(name, val)| {
            val.filter(|s| !s.is_empty())
                .map(|v| format!("{}={}", name, urlencode(v)))
        })
        .collect::<Vec<_>>()
        .join("&")
}

pub(super) fn annotate_options(
    options: &FilterOptions,
    filter: &TraceFilter<'_>,
) -> TraceFilterOptionsView {
    TraceFilterOptionsView {
        users: annotate_group(&options.users, filter.user_id),
        agents: annotate_group(&options.agents, filter.agent_id),
        agent_scopes: annotate_group(&options.agent_scopes, filter.agent_scope),
        policies: annotate_group(&options.policies, filter.policy),
        decisions: annotate_group(&options.decisions, filter.decision),
    }
}

fn annotate_group(items: &[FilterOption], selected: Option<&str>) -> Vec<AnnotatedOption> {
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

pub(super) fn build_pagination(
    query: &TraceListQuery,
    page: i64,
    total_pages: i64,
    page_size: i64,
    total_rows: i64,
    shown_rows: i64,
) -> Pagination {
    let qs = preserved_query_string(query, &["page"]);
    let prefix = if qs.is_empty() {
        format!("{BASE_URL}?")
    } else {
        format!("{BASE_URL}?{qs}&")
    };
    let prev_url = (page > 0).then(|| format!("{prefix}page={}", page - 1));
    let next_url = (page + 1 < total_pages).then(|| format!("{prefix}page={}", page + 1));
    let first_row = if shown_rows == 0 { 0 } else { page * page_size + 1 };
    Pagination {
        current_page: page + 1,
        total_pages,
        first_row,
        last_row: page * page_size + shown_rows,
        total_rows,
        noun: "traces",
        has_prev: prev_url.is_some(),
        has_next: next_url.is_some(),
        prev_url,
        next_url,
    }
}

pub(super) fn serde_stats(query: &TraceListQuery, s: &TraceStats) -> StatsView {
    StatsView {
        total_traces: s.total_traces,
        error_count: s.error_count,
        deny_count: s.deny_count,
        deny_url: toggle_flag_url(query, "deny_only"),
        error_url: toggle_flag_url(query, "error_only"),
        deny_active: query.deny_only.as_deref() == Some("true"),
        error_active: query.error_only.as_deref() == Some("true"),
        cost_display: super::rows::format_cost(s.total_cost_microdollars),
        tokens_display: super::rows::format_token_total(s.total_tokens),
        p50_display: super::rows::format_duration(s.p50_active_ms),
        p95_display: super::rows::format_duration(s.p95_active_ms),
        p99_display: super::rows::format_duration(s.p99_active_ms),
    }
}

/// The deny / error stat cards double as filters: clicking one narrows the list
/// to exactly the traces it counts, clicking it again clears the flag.
fn toggle_flag_url(query: &TraceListQuery, flag: &str) -> String {
    let already_on = match flag {
        "deny_only" => query.deny_only.as_deref() == Some("true"),
        _ => query.error_only.as_deref() == Some("true"),
    };
    let qs = preserved_query_string(query, &[flag, "page"]);
    if already_on {
        return if qs.is_empty() {
            BASE_URL.to_owned()
        } else {
            format!("{BASE_URL}?{qs}")
        };
    }
    if qs.is_empty() {
        format!("{BASE_URL}?{flag}=true")
    } else {
        format!("{BASE_URL}?{qs}&{flag}=true")
    }
}

/// The five columns the list query can actually order by. Each header renders
/// as a link that flips direction when it is already active, so the
/// `cursor:pointer` the table CSS has always shown finally does something.
pub(super) fn build_sort_headers(
    query: &TraceListQuery,
    active_col: &str,
    active_dir: &str,
) -> SortHeaders {
    // Every sort link carries the current filters and time range, minus the
    // sort state it is replacing and the page it would invalidate.
    let qs = preserved_query_string(query, &["sort", "dir", "page"]);
    let prefix = if qs.is_empty() {
        format!("{BASE_URL}?")
    } else {
        format!("{BASE_URL}?{qs}&")
    };
    let header = |key: &str, label: &'static str, class: &'static str| {
        let active = key == active_col;
        // An active column toggles; an inactive one opens largest-first (and
        // newest-first for time), which is what an operator scans for.
        let next_dir = if active && active_dir == "desc" {
            "asc"
        } else {
            "desc"
        };
        SortHeader {
            label,
            class,
            url: format!("{prefix}sort={key}&dir={next_dir}"),
            active,
            aria_sort: if active {
                if active_dir == "asc" {
                    "ascending"
                } else {
                    "descending"
                }
            } else {
                "none"
            },
            indicator: if active {
                if active_dir == "asc" { "▲" } else { "▼" }
            } else {
                "↕"
            },
        }
    };
    SortHeaders {
        started: header("started_at", "Started", "col-started"),
        activity: header("spans", "Activity", "col-spans"),
        tokens: header("tokens", "Tokens", "col-tokens"),
        cost: header("cost", "Cost", "col-cost"),
        duration: header("duration", "Duration", "col-duration"),
    }
}
