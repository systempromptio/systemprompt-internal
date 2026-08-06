//! Odoo MCP server.
//!
//! Odoo (self-hosted Community) is the system of record for CRM, and this
//! server is the only route an agent takes to it. It speaks Odoo's JSON-RPC
//! external API directly — there is no vendor SDK and no proxy service in
//! between.
//!
//! **Every call executes as the calling user.** The server holds no service
//! account. [`identity`] resolves the platform caller to their own Odoo login
//! and API key, stored encrypted in `odoo_identity` when they linked their
//! account on the profile page, and [`client`] issues `execute_kw` with that
//! credential. So Odoo's record rules decide what an agent can read, Odoo's
//! audit log names the real person behind every change, and a user who has not
//! linked their account is told to — never quietly served someone else's data.
//!
//! The tool surface ([`tools`]) covers leads, partners, chatter notes and
//! activities, plus one composite briefing that aggregates in Odoo rather than
//! pulling records back to count them.

pub mod client;
pub mod error;
pub mod format;
pub mod identity;
pub mod server;
pub mod tools;

pub use server::OdooServer;
