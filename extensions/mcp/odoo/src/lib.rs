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
//! The tool surface ([`tools`]) covers leads, partners, activities, and the
//! record-anchored knowledge bank — chatter notes and attachments — plus one
//! composite briefing that aggregates in Odoo rather than pulling records back
//! to count them.
//!
//! There is no separate knowledge store because Odoo Community has no Knowledge
//! app; that is an Enterprise module. What it does have is `mail.message` and
//! `ir.attachment`, both anchored to a `(res_model, res_id)` pair, so every
//! note and every file is already filed against the lead or partner it
//! concerns. [`server::notes`] and [`server::attachments`] read that as the
//! knowledge plane it effectively is.

pub mod apps;
pub mod attachment;
pub mod client;
pub mod error;
pub mod format;
pub mod identity;
pub mod resolve;
pub mod server;
pub mod text;
pub mod tools;

pub use server::OdooServer;
