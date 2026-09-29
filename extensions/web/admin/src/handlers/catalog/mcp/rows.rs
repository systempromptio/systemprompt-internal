//! Assembling the `/admin/mcp` list: joining declaration to runtime, deriving
//! status, and ordering.
//!
//! The join is a full outer one in spirit. Every declared server appears even
//! with no traffic, and every server the runtime tables name appears even when
//! nothing declares it — the second case is an operator problem (a binary
//! serving under a name the catalog does not know) and hiding it would be the
//! one failure this page must not have.
//!
//! The one exception is a name that is not a server at all. The client's hook
//! and the gateway record builtin tool calls under their own source name, so
//! `hook_claude_code` and `gateway` turn up in the executions table as if they
//! were servers. Those are counted once as a footnote and never listed.

use std::collections::HashMap;

use chrono::{DateTime, Utc};
use systemprompt::models::mcp::ExecutionSource;

use crate::handlers::catalog::sorting::apply_direction;
use crate::handlers::ssr::format::format_duration_ms;
use crate::handlers::ssr::ssr_tools::rows::format_bytes;
use crate::repositories::mcp::runtime::{McpConnectionCount, McpServerActivity};
use crate::repositories::overview::liveness::{HEARTBEAT_INTERVAL_SECS, liveness_state};
use crate::types::McpServerDetail;

use super::view::McpServerRow;
use crate::numeric::{percentage, round_to_i64};

pub(super) const BASE_URL: &str = "/admin/mcp";

// Why: the runtime facts for one server, keyed by the name the runtime used.
// Two sources, deliberately separate: who is attached comes from the session
// tables, what happened from the executions table.
pub(crate) struct Runtime {
    pub connections: HashMap<String, McpConnectionCount>,
    pub activity: HashMap<String, McpServerActivity>,
    // Why: calls recorded under a source name rather than a server name —
    // builtin tools the client or gateway executed. Set aside so the fleet
    // rows, tiles and error rate describe servers only.
    pub builtin_calls: i64,
}

impl Runtime {
    pub(crate) fn new(
        connections: Vec<McpConnectionCount>,
        activity: Vec<McpServerActivity>,
    ) -> Self {
        let (builtin, servers): (Vec<_>, Vec<_>) = activity
            .into_iter()
            .partition(|a| ExecutionSource::parse(&a.server_name).is_some());
        Self {
            connections: connections
                .into_iter()
                .map(|c| (c.server_name.clone(), c))
                .collect(),
            activity: servers
                .into_iter()
                .map(|a| (a.server_name.clone(), a))
                .collect(),
            builtin_calls: builtin.iter().map(|a| a.calls).sum(),
        }
    }

    // Why: every name any runtime table knows, so a server serving under a
    // name the catalog never declared is still listed rather than dropped.
    pub(crate) fn names(&self) -> Vec<String> {
        let mut out: Vec<String> = self
            .connections
            .keys()
            .chain(self.activity.keys())
            .cloned()
            .collect();
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn knows(&self, name: &str) -> bool {
        self.connections.contains_key(name) || self.activity.contains_key(name)
    }
}

// Why: the declaration decides the first two answers and the traffic decides
// the rest. A server nothing declares, or one declared and switched off, is not
// a liveness question at all — asking the heartbeat about it would report
// "Idle" for a server that is off on purpose.
//
// The liveness third is `overview::liveness`'s rule verbatim, interval and all,
// so the dashboard strip and this table cannot disagree about the same server.
pub fn status_of(
    configured: bool,
    enabled: bool,
    heartbeat: Option<DateTime<Utc>>,
) -> (&'static str, &'static str) {
    if !configured {
        return ("Unconfigured", "warn");
    }
    if !enabled {
        return ("Disabled", "muted");
    }
    let state = liveness_state(Utc::now(), heartbeat, HEARTBEAT_INTERVAL_SECS);
    (state.label(), state.tone())
}

pub(super) fn error_rate(calls: i64, errors: i64) -> (String, &'static str) {
    if calls == 0 {
        return ("\u{2014}".to_owned(), "muted");
    }
    let pct = percentage(errors, calls);
    let tone = if pct >= 10.0 {
        "err"
    } else if pct > 0.0 {
        "warn"
    } else {
        "ok"
    };
    (format!("{pct:.1}%"), tone)
}

pub(super) fn delta(calls: i64, prior: i64) -> (String, &'static str) {
    if prior == 0 {
        return (String::new(), "");
    }
    let pct = percentage(calls - prior, prior);
    if pct.abs() < 0.5 {
        return ("0%".to_owned(), "");
    }
    let dir = if pct > 0.0 { "up" } else { "down" };
    (format!("{pct:+.0}%"), dir)
}

pub(super) fn ms_display(ms: Option<f64>) -> String {
    ms.map_or_else(
        || "\u{2014}".to_owned(),
        |v| format_duration_ms(round_to_i64(v)),
    )
}

pub(super) fn when_display(t: Option<DateTime<Utc>>) -> String {
    t.map_or_else(
        || "\u{2014}".to_owned(),
        crate::handlers::ssr::format::local_time,
    )
}

// Why: "12/13" reads as the share of calls the client's hook also
// confirmed; attested is a subset of calls, so no calls means no figure.
pub(super) fn attested_display(attested: i64, calls: i64) -> String {
    if calls == 0 {
        return "\u{2014}".to_owned();
    }
    format!("{attested}/{calls}")
}

fn auth_label(server: Option<&McpServerDetail>) -> String {
    let Some(server) = server else {
        return "unknown".to_owned();
    };
    if !server.oauth_required {
        return "open".to_owned();
    }
    let audience = if server.oauth_audience.is_empty() {
        "any"
    } else {
        server.oauth_audience.as_str()
    };
    if server.oauth_scopes.is_empty() {
        return format!("aud {audience}");
    }
    format!("aud {audience} / {}", server.oauth_scopes.join(" "))
}

fn transport_of(server: Option<&McpServerDetail>) -> String {
    match server {
        None => "\u{2014}".to_owned(),
        Some(s) if !s.endpoint.is_empty() => s.endpoint.clone(),
        Some(s) => format!("localhost:{}", s.port),
    }
}

pub(crate) struct RowInputs<'a> {
    pub id: &'a str,
    pub server: Option<&'a McpServerDetail>,
    pub runtime: &'a Runtime,
    pub plugin_count: usize,
    pub assignment_count: i64,
}

pub(crate) fn build_row(input: &RowInputs<'_>) -> McpServerRow {
    let connection = input.runtime.connections.get(input.id);
    let activity = input.runtime.activity.get(input.id);
    let configured = input.server.is_some();
    let enabled = input.server.is_some_and(|s| s.enabled);

    // Why: the last time anything got through — a connection touch or an
    // observed call, whichever is later. Either one proves the server answered.
    let last_seen = connection.and_then(|c| c.last_seen);
    let last_call_at = activity.and_then(|a| a.last_call_at);
    let last_spoke = last_seen.max(last_call_at);
    let (status_label, status_tone) = status_of(configured, enabled, last_spoke);

    let calls = activity.map_or(0, |a| a.calls);
    let failures = activity.map_or(0, |a| a.failures);
    let timeouts = activity.map_or(0, |a| a.timeouts);
    let errors = failures + timeouts;
    let attested = activity.map_or(0, |a| a.attested);
    let prior_calls = activity.map_or(0, |a| a.prior_calls);
    let payload_bytes = activity.map_or(0, |a| a.payload_bytes);
    let (error_rate_display, error_tone) = error_rate(calls, errors);
    let (delta_display, delta_dir) = delta(calls, prior_calls);

    McpServerRow {
        id: input.id.to_owned(),
        description: input
            .server
            .map(|s| s.description.clone())
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| "Not declared in services/mcp.".to_owned()),
        detail_url: format!("{BASE_URL}/{}", input.id),
        access_url: super::super::view::access_url(&super::super::view::mcp_url(input.id)),
        source_path: input
            .server
            .map(|s| s.source_path.clone())
            .unwrap_or_default(),
        configured,
        enabled,
        status_label,
        status_tone,
        server_type: input
            .server
            .map_or_else(|| "\u{2014}".to_owned(), |s| s.server_type.clone()),
        transport: transport_of(input.server),
        auth_label: auth_label(input.server),
        oauth_required: input.server.is_some_and(|s| s.oauth_required),
        alive: liveness_state(Utc::now(), last_spoke, HEARTBEAT_INTERVAL_SECS).is_alive(),
        connections: connection.map_or(0, |c| c.connections),
        connected_users: connection.map_or(0, |c| c.distinct_users),
        last_seen_display: when_display(last_seen),
        calls,
        prior_calls,
        succeeded: activity.map_or(0, |a| a.succeeded),
        failures,
        timeouts,
        errors,
        error_rate_display,
        error_tone,
        attested,
        attested_display: attested_display(attested, calls),
        avg_ms: activity.and_then(|a| a.avg_ms),
        p95_ms: activity.and_then(|a| a.p95_ms),
        avg_display: ms_display(activity.and_then(|a| a.avg_ms)),
        p95_display: ms_display(activity.and_then(|a| a.p95_ms)),
        distinct_users: activity.map_or(0, |a| a.distinct_users),
        distinct_sessions: activity.map_or(0, |a| a.distinct_sessions),
        distinct_tools: activity.map_or(0, |a| a.distinct_tools),
        payload_bytes,
        payload_display: format_bytes(payload_bytes),
        secret_redactions: activity.map_or(0, |a| a.secret_redactions),
        delta_display,
        delta_dir,
        last_call_at,
        last_call_display: when_display(last_call_at),
        plugin_count: input.plugin_count,
        assignment_count: input.assignment_count,
    }
}

// Why: Order the assembled rows by the requested column.
pub(super) fn sort_rows(rows: &mut [McpServerRow], key: &str, dir: &str) {
    match key {
        "status" => apply_direction(rows, dir, |a, b| {
            a.alive.cmp(&b.alive).then_with(|| a.id.cmp(&b.id))
        }),
        "connected" => apply_direction(rows, dir, |a, b| a.connections.cmp(&b.connections)),
        "tools" => apply_direction(rows, dir, |a, b| a.distinct_tools.cmp(&b.distinct_tools)),
        "users" => apply_direction(rows, dir, |a, b| a.distinct_users.cmp(&b.distinct_users)),
        "errors" => apply_direction(rows, dir, |a, b| a.errors.cmp(&b.errors)),
        "p95" => apply_direction(rows, dir, |a, b| {
            a.p95_ms
                .unwrap_or(-1.0)
                .total_cmp(&b.p95_ms.unwrap_or(-1.0))
        }),
        "attested" => apply_direction(rows, dir, |a, b| a.attested.cmp(&b.attested)),
        "last" => apply_direction(rows, dir, |a, b| a.last_call_at.cmp(&b.last_call_at)),
        "grants" => apply_direction(rows, dir, |a, b| {
            a.assignment_count.cmp(&b.assignment_count)
        }),
        "id" => apply_direction(rows, dir, |a, b| a.id.cmp(&b.id)),
        _ => apply_direction(rows, dir, |a, b| a.calls.cmp(&b.calls)),
    }
}
