//! Outbound email MCP server.
//!
//! One tool, `email_send`, and the whole point of it is that it will not send
//! without a human. It is the only tool on this instance that reaches outside
//! the company, so the approval is not an affordance layered on top — it is the
//! control flow.
//!
//! A call is answered in two MRTR rounds (SEP-2322). The first returns a draft
//! preview artifact and an `inputRequests` elicitation asking the person who
//! drafted it to confirm the text. The second runs the `require_approval`
//! governance stage, which may hold the call for a *different* human to resolve
//! at `/admin/governance/approvals`. Only past both does anything reach the
//! relay. A client that does not implement MRTR never gets past round one and
//! never sends — the tool fails closed by construction, with no fallback path
//! to work around it.
//!
//! Sending is SMTP ([`systemprompt_email`]); the CRM trace is a separate step
//! afterwards ([`odoo_log`]), because Odoo is the system of record for anything
//! concerning a lead or a partner.

pub mod draft;
pub mod error;
pub mod odoo_log;
pub mod outbox;
pub mod server;
pub mod tools;

pub use error::EmailToolError;
pub use server::EmailServer;
