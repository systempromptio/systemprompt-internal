//! Conversations — the deterministic record of one gateway context.
//!
//! A conversation is a gateway context; `conversation_facts` (schema 45) is
//! its deterministic record, rolled up from the request log, the hook plane,
//! the tool ledger and the governance spine, and `conversation_analyses`
//! (schema 37) is the judge's one label on top. `detail` reads the fact row,
//! `planes` the turns, tool calls, decisions, skills and safety findings
//! beside it, and `hook_events` what the harness reported; the conversation
//! export assembles all three into one document.

pub mod detail;
pub mod hook_events;
pub mod planes;
mod row;

pub use row::{ContinuationLink, ConversationFactRow};
