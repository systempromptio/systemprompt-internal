//! The raw `list_traces` result row and its lift into [`TraceSummary`].
//!
//! Split from `list.rs` so the query module holds the SQL and nothing else.

use systemprompt::identifiers::{AgentId, SessionId, TraceId, UserId};

use super::TraceSummary;

#[derive(Debug)]
pub(super) struct TraceRow {
    session_id: SessionId,
    trace_id: Option<TraceId>,
    started_at: chrono::DateTime<chrono::Utc>,
    ended_at: chrono::DateTime<chrono::Utc>,
    active_ms: i64,
    window_ms: i64,
    user_id: Option<UserId>,
    user_label: Option<String>,
    agent_id: Option<AgentId>,
    agent_scope: Option<String>,
    model: Option<String>,
    provider: Option<String>,
    span_count: i64,
    request_count: i64,
    tool_call_count: i64,
    governance_count: i64,
    deny_count: i64,
    total_tokens: i64,
    input_tokens: i64,
    output_tokens: i64,
    total_cost_microdollars: i64,
    total_latency_ms: i64,
    cache_hit_any: bool,
    top_tool: Option<String>,
    has_error: bool,
    has_deny: bool,
    total_count: i64,
}

impl From<TraceRow> for TraceSummary {
    fn from(r: TraceRow) -> Self {
        Self {
            session_id: r.session_id,
            trace_id: r.trace_id,
            started_at: r.started_at,
            ended_at: r.ended_at,
            active_ms: r.active_ms,
            window_ms: r.window_ms,
            user_id: r.user_id,
            user_label: r.user_label,
            agent_id: r.agent_id,
            agent_scope: r.agent_scope,
            model: r.model,
            provider: r.provider,
            span_count: r.span_count,
            request_count: r.request_count,
            tool_call_count: r.tool_call_count,
            governance_count: r.governance_count,
            deny_count: r.deny_count,
            total_tokens: r.total_tokens,
            input_tokens: r.input_tokens,
            output_tokens: r.output_tokens,
            total_cost_microdollars: r.total_cost_microdollars,
            total_latency_ms: r.total_latency_ms,
            cache_hit_any: r.cache_hit_any,
            top_tool: r.top_tool,
            has_error: r.has_error,
            has_deny: r.has_deny,
        }
    }
}
