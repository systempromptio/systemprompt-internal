//! Assembling `/admin/mcp/{id}`: the tools, the callers, the call log, the
//! attached connections and the declaration. Who may reach it is the shared
//! "Who gets this" panel.
//!
//! The page answers four questions in that order — what it serves, what it did
//! and for whom, who is on it, and who may reach it — because that is the
//! order an operator asks them when a server misbehaves. The declaration comes
//! last: it is the thing they will go and edit once the first four have told
//! them what is wrong.

use crate::handlers::ssr::entity_urls::{context_detail_url, session_detail_url};
use crate::handlers::ssr::format::{format_duration_ms, local_time, short_id};
use crate::handlers::ssr::ssr_tools::rows::format_bytes;
use crate::numeric::{percentage, round_to_i64};
use crate::repositories::mcp::runtime::{
    McpCallerRow, McpConnectionRow, McpExecutionRow, McpToolStat,
};
use crate::repositories::overview::liveness::{HEARTBEAT_INTERVAL_SECS, liveness_state};
use crate::types::McpServerDetail;
use systemprompt::identifiers::McpExecutionId;
use systemprompt::models::mcp::ExecutionSource;

use super::view::{
    ConfigFactView, McpCallerRowView, McpConnectionRowView, McpExecutionRowView, McpToolRow,
};

fn error_rate(calls: i64, failures: i64) -> (String, &'static str) {
    if calls == 0 {
        return ("\u{2014}".to_owned(), "muted");
    }
    let pct = percentage(failures, calls);
    let tone = if pct >= 10.0 {
        "err"
    } else if pct > 0.0 {
        "warn"
    } else {
        "ok"
    };
    (format!("{pct:.1}%"), tone)
}

fn ms(value: Option<f64>) -> String {
    value.map_or_else(
        || "\u{2014}".to_owned(),
        |v| format_duration_ms(round_to_i64(v)),
    )
}

pub(super) fn tool_rows(stats: Vec<McpToolStat>) -> Vec<McpToolRow> {
    stats
        .into_iter()
        .map(|s| {
            let (error_rate_display, error_tone) = error_rate(s.calls, s.failures);
            McpToolRow {
                tool_name: s.tool_name,
                calls: s.calls,
                failures: s.failures,
                error_rate_display,
                error_tone,
                attested: s.attested,
                distinct_users: s.distinct_users,
                avg_display: ms(s.avg_ms),
                p95_display: ms(s.p95_ms),
                max_display: s.max_ms.map_or_else(
                    || "\u{2014}".to_owned(),
                    |v| format_duration_ms(i64::from(v)),
                ),
                payload_display: format_bytes(s.payload_bytes),
                last_call_display: s
                    .last_call_at
                    .map_or_else(|| "\u{2014}".to_owned(), local_time),
            }
        })
        .collect()
}

pub(super) fn caller_rows(rows: Vec<McpCallerRow>) -> Vec<McpCallerRowView> {
    rows.into_iter()
        .map(|r| McpCallerRowView {
            user_url: format!("/admin/users/{}", r.user_id),
            caller: r.user_label,
            calls: r.calls,
            failures: r.failures,
            distinct_tools: r.distinct_tools,
            last_call_display: r
                .last_call_at
                .map_or_else(|| "\u{2014}".to_owned(), local_time),
        })
        .collect()
}

// Why: the badge says which vantage point recorded the call. A hook-only
// row is a call the client attested and no server observed (an in-process
// tool a hook alone reported), which is its own finding.
fn source_badge(source: &str) -> (&'static str, &'static str) {
    match ExecutionSource::parse(source) {
        Some(ExecutionSource::InProcess) => ("server", "ok"),
        Some(ExecutionSource::Proxy) => ("proxy", "ok"),
        Some(ExecutionSource::Gateway) => ("gateway", "info"),
        Some(ExecutionSource::HookClaudeCode | ExecutionSource::HookOpenCode) => {
            ("hook only", "muted")
        },
        None => ("unknown", "muted"),
    }
}

fn execution_tone(status: &str) -> &'static str {
    match status {
        "success" => "ok",
        "failed" => "err",
        "timeout" => "warn",
        _ => "muted",
    }
}

// Why: the trace page resolves an execution by its trace id or by its own
// id, so a call with no gateway trace still links — by execution id — rather
// than emitting a trace link that answers 404 or none at all.
fn execution_trace_url(trace_id: Option<&str>, execution: &McpExecutionId) -> String {
    format!(
        "/admin/traces/{}",
        urlencoding::encode(trace_id.unwrap_or(execution.as_str()))
    )
}

pub(super) fn execution_rows(rows: Vec<McpExecutionRow>) -> Vec<McpExecutionRowView> {
    rows.into_iter()
        .map(|r| {
            let session = r
                .session_id
                .as_ref()
                .map(|s| s.as_str().to_owned())
                .unwrap_or_default();
            let (source_label, source_tone) = source_badge(&r.source);
            McpExecutionRowView {
                short_id: short_id(r.execution_id.as_str()),
                source_label,
                source_tone,
                status_tone: execution_tone(&r.status),
                started_display: local_time(r.started_at),
                duration_display: r.execution_time_ms.map_or_else(
                    || "\u{2014}".to_owned(),
                    |v| format_duration_ms(i64::from(v)),
                ),
                user_url: format!("/admin/users/{}", r.user_id),
                session_url: r.session_id.as_ref().map(session_detail_url),
                context_url: r.context_id.as_ref().map(context_detail_url),
                trace_url: Some(execution_trace_url(r.trace_id.as_deref(), &r.execution_id)),
                payload_display: format_bytes(r.payload_bytes.unwrap_or(0)),
                session,
                caller: r.user_label,
                error_message: r.error_message.unwrap_or_default(),
                execution_id: r.execution_id,
                tool_name: r.tool_name,
                source: r.source,
                status: r.status,
            }
        })
        .collect()
}

pub(super) fn connection_rows(rows: Vec<McpConnectionRow>) -> Vec<McpConnectionRowView> {
    let now = chrono::Utc::now();
    rows.into_iter()
        .map(|r| {
            // Why: an expired attachment is gone however recently it spoke; a
            // live one that has not spoken lately is still attached but idle.
            let (status_label, status_tone) = if !r.connected {
                ("Expired", "muted")
            } else if liveness_state(now, r.last_seen, HEARTBEAT_INTERVAL_SECS).is_alive() {
                ("Active", "ok")
            } else {
                ("Connected", "info")
            };
            let caller = r
                .user_label
                .or_else(|| r.user_id.as_ref().map(|u| u.as_str().to_owned()))
                .unwrap_or_else(|| "\u{2014}".to_owned());
            McpConnectionRowView {
                short_id: short_id(r.session_id.as_str()),
                user_url: r
                    .user_id
                    .as_ref()
                    .map_or_else(String::new, |u| format!("/admin/users/{u}")),
                caller,
                kind: if r.kind == "external" {
                    "upstream".to_owned()
                } else {
                    "in-process".to_owned()
                },
                client_name: r.client_name.unwrap_or_else(|| "\u{2014}".to_owned()),
                connected: r.connected,
                status_label,
                status_tone,
                last_seen_display: r
                    .last_seen
                    .map_or_else(|| "\u{2014}".to_owned(), local_time),
                expires_display: local_time(r.expires_at),
                session: r.session_id.as_str().to_owned(),
            }
        })
        .collect()
}

// Why: The declaration, flattened into the lines the summary renders.
pub(super) fn config_facts(server: Option<&McpServerDetail>) -> Vec<ConfigFactView> {
    let Some(s) = server else {
        return vec![ConfigFactView {
            label: "Declaration",
            value: "None. This server is known only from its runtime traffic.".to_owned(),
            mono: false,
        }];
    };
    let fact =
        |label: &'static str, value: String, mono: bool| ConfigFactView { label, value, mono };
    let mut out = vec![
        fact("Type", s.server_type.clone(), false),
        fact(
            "Enabled",
            if s.enabled { "yes" } else { "no" }.to_owned(),
            false,
        ),
        fact("Port", s.port.to_string(), true),
    ];
    if !s.endpoint.is_empty() {
        out.push(fact("Endpoint", s.endpoint.clone(), true));
    }
    if !s.binary.is_empty() {
        out.push(fact("Binary", s.binary.clone(), true));
    }
    if !s.package_name.is_empty() {
        out.push(fact("Package", s.package_name.clone(), true));
    }
    out.push(fact(
        "OAuth",
        if s.oauth_required {
            "required"
        } else {
            "not required"
        }
        .to_owned(),
        false,
    ));
    out.push(fact(
        "Audience",
        if s.oauth_audience.is_empty() {
            "\u{2014}".to_owned()
        } else {
            s.oauth_audience.clone()
        },
        true,
    ));
    out.push(fact("Source", s.source_path.clone(), true));
    out
}
