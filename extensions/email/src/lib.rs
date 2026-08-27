//! SMTP transport for outbound mail.
//!
//! A leaf crate: no database, no HTTP, no MCP. It knows how to read the
//! deployment's SMTP secrets and put a message on the wire, and nothing else.
//! The MCP server that decides *whether* to send lives in
//! `extensions/mcp/email`.

pub mod error;
mod service;

pub use error::EmailError;
pub use service::{EmailService, OutboundMessage, mint_message_id};
