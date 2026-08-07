//! The company knowledge-bank MCP server.
//!
//! A persistent store of meeting transcripts, documents and notes on the
//! tenant Postgres, searchable over Postgres full text. The bank starts empty
//! and grows only by `upload_document`; there are no seeded fixtures, because
//! a knowledge bank that answers with invented context is worse than one that
//! answers with nothing.
//!
//! - [`schema`] — the `knowledge_documents` DDL and its registration.
//! - [`store`] — every query the tools run, plus the pure query-shaping helpers
//!   (mode selection, limit clamp, size cap) they depend on.
//! - [`tools`] — the wire contract: `search_project_context`, `list_documents`,
//!   `upload_document`.
//! - [`server`] — the rmcp handler, RBAC, and the admin gate on uploads.

pub mod error;
pub mod schema;
pub mod server;
pub mod store;
pub mod tools;

pub use server::KnowledgeBankServer;
