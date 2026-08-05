//! Stub knowledge-bank MCP server.
//!
//! Stands in for Astound's project-context RAG MCP (workshop transcripts, Jira
//! tickets, Confluence pages) until the real endpoint is connected. Serves
//! keyword search over seeded fixtures via [`store::KnowledgeStore`]; the tool
//! surface ([`tools`]) is the contract the real server replaces — swapping to
//! it is a config-only change in `services/mcp/knowledge-bank.yaml`.

pub mod error;
pub mod server;
pub mod store;
pub mod tools;

pub use server::KnowledgeBankServer;
