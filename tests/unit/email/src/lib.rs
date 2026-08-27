//! Unit tests for `systemprompt-mcp-email`'s pure helpers:
//! - draft validation (recipients, subject, body, and the all-or-nothing Odoo
//!   anchor)
//! - the preview card and the plain-text rendering a human approves from
//! - the confirmation request's wire contract with the client

#[cfg(test)]
mod email_draft;
