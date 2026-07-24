//! View-model assembly for the Inference Requests page.
//!
//! Pure functions that turn repository rows + the parsed query into the typed
//! context the `analytics-requests` template consumes: KPI strip, the two
//! charts, the breakdown tables, paged rows, the tab bar, and the URL builders
//! that preserve query state across tabs, pagination, and the time presets.

use urlencoding::encode as urlencode;

use crate::handlers::ssr::types::bar_pct;
use crate::repositories::analytics::request_stats::RequestStats;
use crate::repositories::analytics::requests::{
    BreakdownRow, RequestFilter, RequestRow, RequestSortColumn, RequestSortSpec, SortDir,
};
use crate::util::time_range::TimeRange;

use super::context::{
    BreakdownRowView, BreakdownView, ChipView, PaginationView, RequestListRowView, RequestStatsView,
    RequestsTab, TabLinkView, TimeRangeView,
};
use super::{BASE_URL, RequestsQuery};

pub(super) fn filter_from_query(query: &RequestsQuery) -> RequestFilter {
    RequestFilter {
        user_id: query.user_id.clone().filter(|u| !u.as_str().is_empty()),
        agent_id: query.agent_id.clone().filter(|a| !a.as_str().is_empty()),
        model: empty_to_none(query.model.as_ref()),
        provider: empty_to_none(query.provider.as_ref()),
        status: empty_to_none(query.status.as_ref()),
        search: empty_to_none(query.q.as_ref()),
    }
}

fn empty_to_none(v: Option<&String>) -> Option<String> {
    v.map(String::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
}

pub(super) fn sort_from_query(query: &RequestsQuery) -> RequestSortSpec {
    let column = match query.sort.as_deref() {
        Some("cost") => RequestSortColumn::Cost,
        Some("latency") => RequestSortColumn::Latency,
        Some("tokens") => RequestSortColumn::Tokens,
        _ => RequestSortColumn::CreatedAt,
    };
    let dir = match query.dir.as_deref() {
        Some("asc") => SortDir::Asc,
        _ => SortDir::Desc,
    };
    RequestSortSpec { column, dir }
}

pub(super) fn stats_to_json(s: &RequestStats) -> RequestStatsView {
    RequestStatsView {
        total: s.total,
        error_count: s.error_count,
        requests_per_minute: format!("{:.2}", s.requests_per_minute),
        p50_latency_ms: s.p50_latency_ms.round() as i64,
        p95_latency_ms: s.p95_latency_ms.round() as i64,
        p99_latency_ms: s.p99_latency_ms.round() as i64,
        total_cost_display: format_cost(Some(s.total_cost_microdollars)),
        error_rate_pct: format!("{:.2}", s.error_rate * 100.0),
        denied_session_count: s.denied_session_count,
        denied_session_rate_pct: format!("{:.2}", s.denied_session_rate * 100.0),
    }
}

// Why: share_pct is against the busiest row rather than the window total, so
// the bars use the full width even when one dimension has a long tail. The
// printed percentage is still share-of-total.
pub(super) fn breakdown_view(
    tab: RequestsTab,
    rows: &[BreakdownRow],
    query: &RequestsQuery,
) -> BreakdownView {
    let (dimension_label, caption, param) = match tab {
        RequestsTab::Providers => (
            "Provider",
            "Traffic, spend, and failures rolled up to the upstream provider.",
            "provider",
        ),
        RequestsTab::Status => (
            "Status",
            "Outcome mix for the window. Failed calls still bill for the tokens they consumed.",
            "status",
        ),
        _ => (
            "Model",
            "Traffic, spend, and failures attributed to the model that produced them.",
            "model",
        ),
    };

    let max = rows.iter().map(|r| r.requests).max().unwrap_or(0);
    let total: i64 = rows.iter().map(|r| r.requests).sum();

    BreakdownView {
        dimension_label,
        caption,
        has_rows: !rows.is_empty(),
        rows: rows
            .iter()
            .map(|r| BreakdownRowView {
                requests: r.requests,
                share_pct: bar_pct(r.requests, max),
                share_display: format!("{:.1}%", pct_of(r.requests, total)),
                tokens_display: format!(
                    "{} / {}",
                    compact_int(r.input_tokens),
                    compact_int(r.output_tokens)
                ),
                cost_display: format_cost(Some(r.cost_microdollars)),
                p50_display: format_ms(r.p50_latency_ms.round() as i64),
                p95_display: format_ms(r.p95_latency_ms.round() as i64),
                error_count: r.error_count,
                error_rate_display: format!("{:.1}%", pct_of(r.error_count, r.requests)),
                has_errors: r.error_count > 0,
                filter_url: log_filter_url(query, param, &r.key),
                key: r.key.clone(),
            })
            .collect(),
    }
}

fn pct_of(part: i64, whole: i64) -> f64 {
    if whole <= 0 {
        return 0.0;
    }
    part as f64 / whole as f64 * 100.0
}

/// Thousands as `12.3k` so a token column stays one line at any magnitude.
fn compact_int(v: i64) -> String {
    if v >= 1_000_000 {
        format!("{:.1}M", v as f64 / 1_000_000.0)
    } else if v >= 10_000 {
        format!("{}k", v / 1000)
    } else if v >= 1000 {
        format!("{:.1}k", v as f64 / 1000.0)
    } else {
        v.to_string()
    }
}

fn format_ms(ms: i64) -> String {
    if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{ms} ms")
    }
}

/// A breakdown row links to the Log tab carrying its own dimension as a filter,
/// on top of whatever filters and window are already active.
fn log_filter_url(query: &RequestsQuery, param: &str, value: &str) -> String {
    let qs = preserved_query_string(query, &[param, "page", "tab"]);
    let mut url = format!("{BASE_URL}?tab=log&{param}={}", urlencode(value));
    if !qs.is_empty() {
        url.push('&');
        url.push_str(&qs);
    }
    url
}

// Why: every tab link carries the current window and the active filters so
// switching tabs never silently resets what the reader chose, but drops the
// page number, which means nothing on another tab.
pub(super) fn tab_links(active: RequestsTab, query: &RequestsQuery, total: i64) -> Vec<TabLinkView> {
    const TABS: [(RequestsTab, &str); 5] = [
        (RequestsTab::Overview, "Overview"),
        (RequestsTab::Models, "Models"),
        (RequestsTab::Providers, "Providers"),
        (RequestsTab::Status, "Status"),
        (RequestsTab::Log, "Log"),
    ];

    let qs = preserved_query_string(query, &["tab", "page"]);
    TABS.iter()
        .map(|&(tab, label)| {
            let mut href = format!("{BASE_URL}?tab={}", tab.as_str());
            if !qs.is_empty() {
                href.push('&');
                href.push_str(&qs);
            }
            TabLinkView {
                slug: tab.as_str(),
                label,
                href,
                is_active: tab == active,
                count: (tab == RequestsTab::Log).then_some(total),
            }
        })
        .collect()
}

// Why: removing a chip drops just that parameter and keeps the tab, window,
// and every other filter intact.
pub(super) fn active_chips(query: &RequestsQuery) -> Vec<ChipView> {
    let mut chips = Vec::new();
    for (param, group_label, value) in [
        ("model", "Model", query.model.as_deref()),
        ("provider", "Provider", query.provider.as_deref()),
        ("status", "Status", query.status.as_deref()),
        ("q", "Search", query.q.as_deref()),
    ] {
        let Some(value) = value.filter(|s| !s.is_empty()) else {
            continue;
        };
        let qs = preserved_query_string(query, &[param, "page"]);
        chips.push(ChipView {
            group_label,
            label: value.to_owned(),
            remove_url: if qs.is_empty() {
                BASE_URL.to_owned()
            } else {
                format!("{BASE_URL}?{qs}")
            },
        });
    }
    chips
}

pub(super) fn request_row_to_json(r: &RequestRow) -> RequestListRowView {
    RequestListRowView {
        id: r.id.clone(),
        request_id: r.request_id.clone(),
        trace_id: r.trace_id.clone(),
        session_id: r.session_id.clone(),
        user_id: r.user_id.clone(),
        user_label: r
            .user_label
            .clone()
            .unwrap_or_else(|| r.user_id.as_str().to_owned()),
        provider: r.provider.clone(),
        model: r.model.clone(),
        status: r.status.clone(),
        is_error: is_error_status(&r.status),
        input_tokens: r.input_tokens,
        output_tokens: r.output_tokens,
        tokens_total: r.input_tokens.unwrap_or(0) + r.output_tokens.unwrap_or(0),
        cost_microdollars: r.cost_microdollars,
        cost_display: format_cost(Some(r.cost_microdollars)),
        latency_ms: r.latency_ms,
        error_message: r.error_message.clone(),
        decision_count: r.decision_count,
        deny_count: r.deny_count,
        is_denied_preflight: r.deny_count > 0,
        tool_call_count: r.tool_call_count,
        created_at: r.created_at.to_rfc3339(),
        created_at_local: r
            .created_at
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M:%S")
            .to_string(),
    }
}

fn is_error_status(status: &str) -> bool {
    !matches!(status, "completed" | "pending" | "streaming")
}

fn format_cost(microdollars: Option<i64>) -> String {
    let Some(m) = microdollars else {
        return "—".to_owned();
    };
    let dollars = m as f64 / 1_000_000.0;
    if dollars == 0.0 {
        "$0".to_owned()
    } else if dollars < 0.01 {
        format!("${dollars:.6}")
    } else {
        format!("${dollars:.4}")
    }
}

pub(super) fn time_range_context(
    query: &RequestsQuery,
    range: &TimeRange,
    auto_widened: Option<&'static str>,
) -> TimeRangeView {
    let preset = query.preset.clone().unwrap_or_else(|| {
        if query.from.is_some() && query.to.is_some() {
            "custom".to_owned()
        } else {
            auto_widened.unwrap_or("24h").to_owned()
        }
    });
    let qs = preserved_query_string(query, &["preset", "from", "to"]);
    let q_suffix = if qs.is_empty() {
        String::new()
    } else {
        format!("&{qs}")
    };
    TimeRangeView {
        preset,
        from: range.from.to_rfc3339(),
        to: range.to.to_rfc3339(),
        base_url: BASE_URL,
        query: q_suffix,
        auto_widened,
    }
}

// Why: Clear drops every filter but keeps the reader on the tab and window
// they are looking at.
pub(super) fn clear_url(query: &RequestsQuery) -> String {
    let qs = preserved_query_string(
        query,
        &["model", "provider", "status", "q", "user_id", "agent_id", "page"],
    );
    if qs.is_empty() {
        BASE_URL.to_owned()
    } else {
        format!("{BASE_URL}?{qs}")
    }
}

fn preserved_query_string(query: &RequestsQuery, drop: &[&str]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let pairs_str: [(&str, Option<&str>); 12] = [
        ("tab", query.tab.as_deref()),
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
        ("model", query.model.as_deref()),
        ("provider", query.provider.as_deref()),
        ("status", query.status.as_deref()),
        ("q", query.q.as_deref()),
        ("sort", query.sort.as_deref()),
        ("dir", query.dir.as_deref()),
    ];
    for (name, value) in pairs_str {
        if drop.contains(&name) {
            continue;
        }
        let Some(v) = value.filter(|s| !s.is_empty()) else {
            continue;
        };
        parts.push(format!("{}={}", name, urlencode(v)));
    }
    if !drop.contains(&"page")
        && let Some(p) = query.page.filter(|p| *p > 0)
    {
        parts.push(format!("page={p}"));
    }
    parts.join("&")
}

pub(super) fn build_pagination(
    query: &RequestsQuery,
    page: i64,
    total_pages: i64,
    page_size: i64,
    total_rows: i64,
    shown_rows: i64,
) -> PaginationView {
    let qs = preserved_query_string(query, &["page"]);
    let prefix = if qs.is_empty() {
        format!("{BASE_URL}?")
    } else {
        format!("{BASE_URL}?{qs}&")
    };
    let prev_url = (page > 0).then(|| format!("{prefix}page={}", page - 1));
    let next_url = (page + 1 < total_pages).then(|| format!("{prefix}page={}", page + 1));
    let first_row = if shown_rows == 0 { 0 } else { page * page_size + 1 };
    PaginationView {
        current_page: page + 1,
        total_pages,
        first_row,
        last_row: page * page_size + shown_rows,
        total_rows,
        noun: if total_rows == 1 { "request" } else { "requests" },
        has_prev: prev_url.is_some(),
        has_next: next_url.is_some(),
        prev_url,
        next_url,
    }
}
