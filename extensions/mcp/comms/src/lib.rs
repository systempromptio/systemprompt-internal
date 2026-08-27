//! The team comms MCP server.
//!
//! Messages between people and their agent sessions, on our own governed
//! infrastructure. The rule the design turns on is that a message reaches a
//! running conversation only when it names that session: `@ed` goes to a
//! person's inbox and never interrupts, `@ed/odoo-crm` reaches exactly one
//! session, `#crm` reaches a channel.
//!
//! - [`schema`] — the four `comms_*` tables and their registration.
//! - [`store`] — address parsing, the delivery-class decision, and every query.
//! - [`tools`] — the wire contract for the five tools.
//! - [`server`] — the rmcp handler, RBAC, and dispatch.

pub mod error;
pub mod schema;
pub mod server;
pub mod store;
pub mod tools;

pub use server::CommsServer;
