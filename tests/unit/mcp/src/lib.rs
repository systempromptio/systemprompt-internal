//! Unit tests for the MCP extension crates' pure helpers:
//! - `systemprompt-mcp-agent`'s `filter_hallucinated_args` (CLI arg scrubbing)
//! - `systemprompt-mcp-shared`'s `truncate_on_char_boundary` (rejection-reason
//!   truncation with UTF-8 safety) and `AuditMetadata`'s stored JSON shape
//! - `systemprompt-mcp-agent`'s `systemprompt` tool contract (single tool, its
//!   input/output schema) and its error type's code / status / retryability
//! - `systemprompt-mcp-knowledge-bank`'s `KnowledgeStore` (fixture seeding,
//!   keyword search, filtered listing, insert), its tool contract, its error
//!   mapping, and its search-result rendering
//! - `systemprompt-mcp-odoo`'s JSON-RPC envelope handling, the search domains
//!   it builds (including the acting-uid filter that makes "my activities"
//!   mean the caller's), its record rendering, its sealed-credential framing,
//!   its tool contract, and its error mapping
//! - the odoo knowledge plane: chatter HTML reduced to readable text,
//!   query-centred snippets, the note-search domain's OR-over-AND shape, and
//!   the attachment size gates on both the upload and the inline-return side

#[cfg(test)]
mod audit_metadata;
#[cfg(test)]
mod document_summary;
#[cfg(test)]
mod filter_hallucinated_args;
#[cfg(test)]
mod knowledge_bank_error;
#[cfg(test)]
mod knowledge_bank_tools;
#[cfg(test)]
mod knowledge_store;
#[cfg(test)]
mod knowledge_store_scoring;
#[cfg(test)]
mod odoo_attachments;
#[cfg(test)]
mod odoo_credentials;
#[cfg(test)]
mod odoo_domains;
#[cfg(test)]
mod odoo_error;
#[cfg(test)]
mod odoo_format;
#[cfg(test)]
mod odoo_notes;
#[cfg(test)]
mod odoo_rpc;
#[cfg(test)]
mod odoo_text;
#[cfg(test)]
mod odoo_tools;
#[cfg(test)]
mod systemprompt_error;
#[cfg(test)]
mod systemprompt_tools;
#[cfg(test)]
mod truncate_on_char_boundary;
