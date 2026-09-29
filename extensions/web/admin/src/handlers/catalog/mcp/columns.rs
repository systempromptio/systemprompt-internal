//! The column set `/admin/mcp` can be ordered by, and the tiles above it.
//!
//! Kept beside the row assembly rather than inside it: the columns are the
//! page's contract with the sort links and the template header row, and the
//! tiles are the same six numbers on the list page and on one server's detail
//! page, which is what lets a single server be read against the fleet.

use crate::handlers::catalog::sorting::SortColumn;
use crate::handlers::ssr::format::short_num;

use super::rows::{delta, error_rate, ms_display};
use super::view::{McpKpiView, McpServerRow};

// Why: The columns `/admin/mcp` can be ordered by.
pub(super) fn columns() -> Vec<SortColumn> {
    vec![
        SortColumn {
            key: "id",
            label: "Server",
            class: "",
            hint: "The id declared in services/mcp, or the name the runtime used",
        },
        SortColumn {
            key: "status",
            label: "Status",
            class: "",
            hint: "Alive when a connection or a call got through inside the heartbeat window",
        },
        SortColumn {
            key: "connected",
            label: "Connected",
            class: "sp-table__cell--num",
            hint: "Unexpired client connections right now, and the people behind them",
        },
        SortColumn {
            key: "calls",
            label: "Calls",
            class: "sp-table__cell--num",
            hint: "Tool executions the server recorded in the window, against the window before",
        },
        SortColumn {
            key: "tools",
            label: "Tools",
            class: "sp-table__cell--num",
            hint: "Distinct tools called in the window",
        },
        SortColumn {
            key: "users",
            label: "Users",
            class: "sp-table__cell--num",
            hint: "Distinct people who called it in the window",
        },
        SortColumn {
            key: "errors",
            label: "Errors",
            class: "sp-table__cell--num",
            hint: "Failed and timed-out calls as a share of the window",
        },
        SortColumn {
            key: "p95",
            label: "p95",
            class: "sp-table__cell--num",
            hint: "95th-percentile execution time in the window",
        },
        SortColumn {
            key: "attested",
            label: "Attested",
            class: "sp-table__cell--num",
            hint: "Calls the client's completion hook also confirmed, as a share of all calls",
        },
        SortColumn {
            key: "last",
            label: "Last",
            class: "sp-table__cell--date",
            hint: "When this server last executed a tool",
        },
        SortColumn {
            key: "grants",
            label: "Grants",
            class: "sp-table__cell--num",
            hint: "Access-control rules naming this server",
        },
    ]
}

// Why: The five headline facts, plus the one that only matters when it is
// wrong.
pub(super) fn kpis(rows: &[McpServerRow]) -> Vec<McpKpiView> {
    let configured = rows.iter().filter(|r| r.configured).count();
    let alive = rows.iter().filter(|r| r.alive).count();
    let connections: i64 = rows.iter().map(|r| r.connections).sum();
    let connected_users: i64 = rows.iter().map(|r| r.connected_users).sum();
    let calls: i64 = rows.iter().map(|r| r.calls).sum();
    let prior: i64 = rows.iter().map(|r| r.prior_calls).sum();
    let errors: i64 = rows.iter().map(|r| r.errors).sum();
    let unconfigured = rows.len() - configured;
    let (rate, rate_tone) = error_rate(calls, errors);
    let (delta_display, _) = delta(calls, prior);

    // Why: one server's p95 is its own; the fleet's is the worst server's,
    // because the tile answers "how slow can a call get".
    let p95 = rows
        .iter()
        .filter_map(|r| r.p95_ms)
        .fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a| a.max(v)))
        });
    let avg = rows
        .iter()
        .filter_map(|r| r.avg_ms)
        .fold(None, |acc: Option<f64>, v| {
            Some(acc.map_or(v, |a| a.max(v)))
        });

    vec![
        McpKpiView {
            label: "Declared",
            value: configured.to_string(),
            note: format!("{unconfigured} serving undeclared"),
            tone: if unconfigured > 0 { "warn" } else { "" },
            unit: "",
        },
        McpKpiView {
            label: "Alive now",
            value: alive.to_string(),
            note: format!("of {configured} declared"),
            tone: if alive == 0 { "warn" } else { "ok" },
            unit: "",
        },
        McpKpiView {
            label: "Connected",
            value: short_num(connections),
            note: format!("{connected_users} people attached"),
            tone: "",
            unit: "",
        },
        McpKpiView {
            label: "Calls",
            value: short_num(calls),
            note: if delta_display.is_empty() {
                "tool executions in the window".to_owned()
            } else {
                format!("{delta_display} vs the window before")
            },
            tone: "",
            unit: "",
        },
        McpKpiView {
            label: "Errors",
            value: short_num(errors),
            note: format!("{rate} of calls"),
            tone: rate_tone,
            unit: "",
        },
        McpKpiView {
            label: "p95 latency",
            value: ms_display(p95),
            note: format!("slowest average {}", ms_display(avg)),
            tone: "",
            unit: "",
        },
    ]
}
