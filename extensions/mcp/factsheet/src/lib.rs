//! # systemprompt-mcp-factsheet
//!
//! MCP surface over the factsheet engine: list the sheets, read one as data,
//! render one to a stored PDF.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.

pub mod error;
pub mod server;
pub mod store;
pub mod tools;

pub use error::{ServerError, ServerResult};
pub use server::FactsheetServer;
