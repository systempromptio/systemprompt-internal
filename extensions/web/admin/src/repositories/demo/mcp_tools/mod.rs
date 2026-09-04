//! MCP tool telemetry: per-invocation rows, the governance rollup, and the
//! user × tool matrix.
//!
//! Hook events name a tool `mcp__<server>__<tool>`. `approval_requests` carries
//! the bare name, and `governance_decisions` carries either: the MCP proxy
//! writes the bare name, the Claude Code govern hook writes the wire name.
//! Every join here normalises both sides to the bare name.

mod invocations;
mod matrix;
mod stats;

pub use invocations::{McpToolInvocationRow, list_mcp_tool_invocations};
pub use matrix::list_user_mcp_tool_matrix;
pub use stats::{McpToolStatRow, list_mcp_tool_stats};
