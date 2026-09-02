//! The company knowledge-bank MCP server.
//!
//! A persistent store of meeting transcripts, documents and notes on the
//! tenant Postgres, searchable over Postgres full text. The bank starts empty
//! and grows only by `upload_document`; there are no seeded fixtures, because
//! a knowledge bank that answers with invented context is worse than one that
//! answers with nothing.
//!
//! The bank is also where `brain@systemprompt.io` lands: the knowledge jobs
//! capture and categorize inbound mail here, and [`proposal`] turns each
//! categorized email into a governed, human-approved projection into Odoo.
//!
//! - [`schema`] — the `knowledge_documents` and `knowledge_odoo_projection` DDL
//!   and their registration.
//! - [`store`] — every query the tools and jobs run, plus the pure
//!   query-shaping helpers (mode selection, limit clamp, size cap).
//! - [`proposal`] — intent, planner, approval hold, and the Odoo executor.
//! - [`tools`] — the wire contract: `search_project_context`, `list_documents`,
//!   `upload_document`, `proposal_list`, `proposal_get`, `proposal_decide`.
//! - [`server`] — the rmcp handler, RBAC, and the admin gate.

pub mod error;
pub mod proposal;
pub mod schema;
pub mod server;
pub mod store;
pub mod tools;

pub use server::KnowledgeBankServer;
