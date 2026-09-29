//! `/admin/mcp` and `/admin/mcp/{id}` — the fleet as the page shows it, the
//! raw call log, and the per-tool split.
//!
//! The fleet rows are the page's own rows
//! (`handlers::catalog::mcp::fleet_rows`), so a downloaded number is the number
//! that was on screen. The call log is every `mcp_tool_executions` row in the
//! window with its `source`, because a data dump must let the reader tell a
//! server-observed run from the client's attestation of it; `?server=` narrows
//! either to one server, as the detail page's export does.

use async_trait::async_trait;
use serde::Deserialize;


use super::requests::time_range;
use crate::error::AdminResult;
use crate::export::model::{Cell, CellKind, Column, DataSet, ExportContext, Table, Window};
use crate::handlers::catalog::mcp::{McpServerRow, fleet_rows};
use crate::repositories::mcp::runtime::{
    CallLogQuery, McpExecutionRow, McpToolStat, list_mcp_executions_paged, list_mcp_tool_stats,
};

pub(crate) struct McpServers;
pub(crate) struct McpCalls;
pub(crate) struct McpTools;

#[derive(Debug, Default, Deserialize)]
struct ServerQuery {
    server: Option<String>,
}

const SERVER_COLUMNS: &[Column] = &[
    Column::new("server", "Server", CellKind::Text),
    Column::new("declared", "Declared", CellKind::Bool),
    Column::new("enabled", "Enabled", CellKind::Bool),
    Column::new("status", "Status", CellKind::Text),
    Column::new("type", "Type", CellKind::Text).optional(),
    Column::new("transport", "Transport", CellKind::Text).optional(),
    Column::new("auth", "Auth", CellKind::Text).optional(),
    Column::new("connections", "Connections", CellKind::Integer),
    Column::new("connected_users", "Connected users", CellKind::Integer),
    Column::new("calls", "Calls", CellKind::Integer),
    Column::new("prior_calls", "Calls (window before)", CellKind::Integer).optional(),
    Column::new("succeeded", "Succeeded", CellKind::Integer),
    Column::new("failed", "Failed", CellKind::Integer),
    Column::new("timeouts", "Timed out", CellKind::Integer),
    Column::new("error_rate", "Error rate", CellKind::Text),
    Column::new("attested", "Attested", CellKind::Integer),
    Column::new("tools", "Tools", CellKind::Integer),
    Column::new("users", "Users", CellKind::Integer),
    Column::new("sessions", "Sessions", CellKind::Integer),
    Column::new("avg_ms", "Avg (ms)", CellKind::Decimal).optional(),
    Column::new("p95_ms", "p95 (ms)", CellKind::Decimal).optional(),
    Column::new("payload_bytes", "Payload bytes", CellKind::Integer).optional(),
    Column::new("secret_redactions", "Secret redactions", CellKind::Integer).optional(),
    Column::new("last_call_at", "Last call", CellKind::Timestamp),
    Column::new("grants", "Grants", CellKind::Integer),
    Column::new("plugins", "Plugins", CellKind::Integer),
    Column::new("source_path", "Declared in", CellKind::Text).optional(),
];

fn server_row(r: &McpServerRow) -> Vec<Cell> {
    vec![
        r.id.as_str().into(),
        r.configured.into(),
        r.enabled.into(),
        r.status_label.into(),
        r.server_type.as_str().into(),
        r.transport.as_str().into(),
        r.auth_label.as_str().into(),
        r.connections.into(),
        r.connected_users.into(),
        r.calls.into(),
        r.prior_calls.into(),
        r.succeeded.into(),
        r.failures.into(),
        r.timeouts.into(),
        r.error_rate_display.as_str().into(),
        r.attested.into(),
        r.distinct_tools.into(),
        r.distinct_users.into(),
        r.distinct_sessions.into(),
        Cell::opt_decimal(r.avg_ms),
        Cell::opt_decimal(r.p95_ms),
        r.payload_bytes.into(),
        r.secret_redactions.into(),
        Cell::opt_time(r.last_call_at),
        r.assignment_count.into(),
        i64::try_from(r.plugin_count).unwrap_or(i64::MAX).into(),
        r.source_path.as_str().into(),
    ]
}

const CALL_COLUMNS: &[Column] = &[
    Column::new("started_at", "Started", CellKind::Timestamp),
    Column::new("completed_at", "Completed", CellKind::Timestamp).optional(),
    Column::new("execution_id", "Execution", CellKind::Text),
    Column::new("server", "Server", CellKind::Text),
    Column::new("tool", "Tool", CellKind::Text),
    Column::new("source", "Seen by", CellKind::Text),
    Column::new("correlation", "Correlation", CellKind::Text).optional(),
    Column::new("status", "Status", CellKind::Text),
    Column::new("duration_ms", "Duration (ms)", CellKind::Integer),
    Column::new("user_id", "User", CellKind::Text),
    Column::new("user_label", "Name", CellKind::Text),
    Column::new("session_id", "Session", CellKind::Text),
    Column::new("context_id", "Conversation", CellKind::Text),
    Column::new("trace_id", "Trace", CellKind::Text).optional(),
    Column::new("ai_tool_call_id", "Tool call id", CellKind::Text).optional(),
    Column::new("payload_bytes", "Payload bytes", CellKind::Integer),
    Column::new("secret_redactions", "Secret redactions", CellKind::Integer).optional(),
    Column::new("error_message", "Error", CellKind::Text),
    Column::new("input", "Input", CellKind::Text).optional(),
];

fn call_row(r: &McpExecutionRow) -> Vec<Cell> {
    vec![
        r.started_at.into(),
        Cell::opt_time(r.completed_at),
        r.execution_id.as_str().into(),
        r.server_name.as_str().into(),
        r.tool_name.as_str().into(),
        r.source.as_str().into(),
        r.correlation.as_str().into(),
        r.status.as_str().into(),
        Cell::opt_int(r.execution_time_ms),
        r.user_id.as_str().into(),
        r.user_label.as_str().into(),
        Cell::opt_text(
            r.session_id
                .as_ref()
                .map(systemprompt::identifiers::SessionId::as_str),
        ),
        Cell::opt_text(
            r.context_id
                .as_ref()
                .map(systemprompt::identifiers::ContextId::as_str),
        ),
        Cell::opt_text(r.trace_id.as_deref()),
        Cell::opt_text(r.ai_tool_call_id.as_deref()),
        Cell::opt_int(r.payload_bytes),
        Cell::opt_int(r.secret_redactions),
        Cell::opt_text(r.error_message.as_deref()),
        r.input.as_str().into(),
    ]
}

const TOOL_COLUMNS: &[Column] = &[
    Column::new("server", "Server", CellKind::Text),
    Column::new("tool", "Tool", CellKind::Text),
    Column::new("calls", "Calls", CellKind::Integer),
    Column::new("failed", "Failed or timed out", CellKind::Integer),
    Column::new("attested", "Attested", CellKind::Integer),
    Column::new("users", "Users", CellKind::Integer),
    Column::new("avg_ms", "Avg (ms)", CellKind::Decimal),
    Column::new("p95_ms", "p95 (ms)", CellKind::Decimal),
    Column::new("max_ms", "Max (ms)", CellKind::Integer),
    Column::new("payload_bytes", "Payload bytes", CellKind::Integer),
    Column::new("last_call_at", "Last call", CellKind::Timestamp),
];

fn tool_row(r: &McpToolStat) -> Vec<Cell> {
    vec![
        r.server_name.as_str().into(),
        r.tool_name.as_str().into(),
        r.calls.into(),
        r.failures.into(),
        r.attested.into(),
        r.distinct_users.into(),
        Cell::opt_decimal(r.avg_ms),
        Cell::opt_decimal(r.p95_ms),
        Cell::opt_int(r.max_ms),
        r.payload_bytes.into(),
        Cell::opt_time(r.last_call_at),
    ]
}

#[async_trait]
impl DataSet for McpServers {
    fn id(&self) -> &'static str {
        "mcp-servers"
    }
    fn title(&self) -> &'static str {
        "MCP servers"
    }
    fn description(&self) -> &'static str {
        "One row per server: declaration, connections, calls, errors, latency and grants over the window."
    }
    fn columns(&self) -> &'static [Column] {
        SERVER_COLUMNS
    }
    fn window(&self) -> Window {
        Window::Live
    }
    async fn load(&self, ctx: &ExportContext<'_>) -> AdminResult<Table> {
        let fleet = fleet_rows(ctx.pool, &ctx.user.roles, time_range(ctx)?).await?;
        Ok(Table::complete(fleet.rows.iter().map(server_row).collect()))
    }
}

#[async_trait]
impl DataSet for McpCalls {
    fn id(&self) -> &'static str {
        "mcp-calls"
    }
    fn title(&self) -> &'static str {
        "MCP tool calls"
    }
    fn description(&self) -> &'static str {
        "Every recorded call in the window, one row per observation; `Seen by` says whether the server or the client recorded it."
    }
    fn columns(&self) -> &'static [Column] {
        CALL_COLUMNS
    }
    fn window(&self) -> Window {
        Window::Live
    }
    async fn load(&self, ctx: &ExportContext<'_>) -> AdminResult<Table> {
        let query: ServerQuery = ctx.query()?;
        let range = time_range(ctx)?;
        let (rows, total) = list_mcp_executions_paged(
            ctx.pool,
            CallLogQuery {
                server_name: query.server.as_deref().filter(|s| !s.is_empty()),
                from: range.from,
                to: range.to,
                limit: ctx.limit,
                offset: 0,
            },
        )
        .await?;
        Ok(Table {
            rows: rows.iter().map(call_row).collect(),
            total,
        })
    }
}

#[async_trait]
impl DataSet for McpTools {
    fn id(&self) -> &'static str {
        "mcp-tools"
    }
    fn title(&self) -> &'static str {
        "MCP tools"
    }
    fn description(&self) -> &'static str {
        "One row per server and tool: calls, errors, attestation, latency and payload over the window."
    }
    fn columns(&self) -> &'static [Column] {
        TOOL_COLUMNS
    }
    fn window(&self) -> Window {
        Window::Live
    }
    async fn load(&self, ctx: &ExportContext<'_>) -> AdminResult<Table> {
        let query: ServerQuery = ctx.query()?;
        let range = time_range(ctx)?;
        let rows = list_mcp_tool_stats(
            ctx.pool,
            query.server.as_deref().filter(|s| !s.is_empty()),
            range.from,
            range.to,
            // Why: one row per server and tool, so the preview's one-row
            // `ctx.limit` would count one tool; the aggregate is read whole.
            self.cap(),
        )
        .await?;
        Ok(Table::complete(rows.iter().map(tool_row).collect()))
    }
}
