//! Runtime state for the MCP servers declared in `services/mcp/*.yaml`.
//!
//! The YAML says what a server *is*; these queries say what it has been
//! *doing*. Three tables carry that: `mcp_external_sessions` and
//! `mcp_sessions` (who is attached right now, for proxied and in-process
//! servers respectively) and `mcp_tool_executions` (every tool call, with its
//! outcome and which side observed it).
//!
//! The rule that turns "last spoke at" into alive/stale/idle does not live
//! here: `repositories::overview::liveness` owns it, and the MCP pages feed it
//! the latest of a server's last connection touch and last observed call.
//!
//! Every function here is keyed by the server name as the runtime records it,
//! which is not guaranteed to be a name the catalog declares. A server that
//! appears only in the executions table is a real thing that really ran, so it
//! is returned rather than filtered out; the pages surface it as unconfigured.

mod call_log;
mod connections;
mod executions;

pub use call_log::{
    CallLogQuery, McpCallerRow, McpExecutionRow, list_mcp_callers, list_mcp_executions_paged,
};
pub use connections::{
    McpConnectionCount, McpConnectionRow, list_mcp_connections, list_mcp_connections_for_server,
};
pub use executions::{
    McpServerActivity, McpToolStat, list_mcp_server_activity, list_mcp_tool_stats,
};
