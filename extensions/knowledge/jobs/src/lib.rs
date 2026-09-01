//! Scheduled knowledge-capture jobs: the brain@ pipeline.
//!
//! - [`EmailIngestionJob`] polls the `brain@systemprompt.io` mailbox over IMAP
//!   and captures every unseen message as a raw `knowledge_documents` row, with
//!   a Message-ID ledger making ingestion idempotent.
//! - [`KnowledgeCategorizationJob`] asks the AI gateway for a category, summary
//!   and `crm_intent` per raw document.
//! - [`KnowledgeProposalJob`] plans what each categorized email should become
//!   in Odoo and opens the approval a human must answer.
//! - [`KnowledgeOdooApplyJob`] settles answered approvals — applying, denying,
//!   expiring — and retries failed applies.
//!
//! No job in this crate writes to Odoo before an admin has approved the
//! specific proposal, and the write runs as that admin.

mod ai;
mod categorization;
mod categorize_output;
mod email_ingestion;
mod error;
mod extension;
mod imap_client;
mod mail;
mod odoo_apply;
mod proposal;
mod registry;

pub use categorization::KnowledgeCategorizationJob;
pub use email_ingestion::EmailIngestionJob;
pub use error::KnowledgeJobError;
pub use extension::KnowledgeJobsExtension;
pub use odoo_apply::KnowledgeOdooApplyJob;
pub use proposal::KnowledgeProposalJob;
pub use registry::{JOB_TAG, extension_jobs};

/// Pure helpers behind the jobs, re-exported for the external test workspace
/// so parsing and document-shaping behaviour can be asserted without IMAP or
/// a database. Not part of the public API — the job structs are.
#[doc(hidden)]
pub mod internals {
    pub use crate::categorize_output::{
        CATEGORIES, Categorization, parse_output, response_schema, structured_json, system_prompt,
        user_prompt,
    };
    pub use crate::mail::{CapturedEmail, captured_from_rfc822, metadata_json, render_document};
}
