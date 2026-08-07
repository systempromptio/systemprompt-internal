//! Scheduled knowledge-capture jobs.
//!
//! One job today: [`EmailIngestionJob`] polls the `brain@systemprompt.io`
//! mailbox over IMAP and captures every unseen message as a raw
//! `knowledge_documents` row (`source = 'email'`, `status = 'raw'`), with a
//! Message-ID ledger making ingestion idempotent. Categorization is a later,
//! separate pass — this crate only captures.

mod categorization;
mod categorize_output;
mod email_ingestion;
mod error;
mod extension;
mod imap_client;
mod mail;
mod registry;

pub use categorization::KnowledgeCategorizationJob;
pub use email_ingestion::EmailIngestionJob;
pub use error::KnowledgeJobError;
pub use extension::KnowledgeJobsExtension;
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
