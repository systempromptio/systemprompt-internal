//! MCP tool telemetry: per-invocation rows, the governance rollup, and the
//! user × tool matrix.
//!
//! Hook events name a tool `mcp__<server>__<tool>`, while
//! `governance_decisions.tool_name` and `approval_requests.tool_name` carry the
//! **bare** tool name. Every join here normalises to the bare name.

mod invocations;
mod matrix;
mod stats;

pub use invocations::{McpToolInvocationRow, list_mcp_tool_invocations};
pub use matrix::list_user_mcp_tool_matrix;
pub use stats::{McpToolStatRow, list_mcp_tool_stats};
