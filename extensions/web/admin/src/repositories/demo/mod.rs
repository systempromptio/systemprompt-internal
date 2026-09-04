//! Demo dashboards: skill adoption, MCP tool usage, and the governance logbook.
//!
//! Every function here reads live — nothing is materialised — because the
//! demo pages are watched while the demo runs and must change as events land.
//! Volumes are hundreds of rows, so a read-time fold is cheaper than the
//! staleness a rollup table would introduce.
//!
//! Token and cost figures are *attributed*, not measured: `ai_requests` carries
//! no tool-call identifier, so usage is joined to an invocation by the
//! same-user time window documented in [`attribution`].

pub mod attribution;
pub mod filter;
pub mod kpis;
pub mod logbook;
pub mod mcp_tools;
pub mod series;
pub mod skill_invocations;
pub mod skill_matrix;

pub use filter::DemoFilter;
pub use skill_matrix::{UsageMatrix, UsageMatrixRow};
