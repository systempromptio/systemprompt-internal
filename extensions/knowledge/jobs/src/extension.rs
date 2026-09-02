//! Extension registration: contributes the email-ingest schema and the
//! knowledge jobs to whatever runtime links this crate.

use std::sync::Arc;

use systemprompt::extension::prelude::{
    Extension, ExtensionMetadata, Migration, SchemaDefinition, extension_migrations,
    register_extension,
};
use systemprompt::traits::Job;

pub(crate) const SCHEMA_EMAIL_INGEST: &str = include_str!("../schema/01_email_ingest.sql");
pub(crate) const MIGRATION_CATEGORIZATION: &str =
    include_str!("../schema/migrations/001_knowledge_document_categorization.sql");
pub(crate) const MIGRATION_PROPOSAL: &str =
    include_str!("../schema/migrations/002_knowledge_document_proposal.sql");

/// Jobs-and-schema extension: no router, no assets, no config.
///
/// The `knowledge_documents` table itself belongs to the knowledge-bank MCP
/// crate; this extension adds the ingest ledger and the pipeline columns
/// (guarded, so ordering against the knowledge-bank schema does not matter).
#[derive(Debug, Default, Clone, Copy)]
pub struct KnowledgeJobsExtension;

impl Extension for KnowledgeJobsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "knowledge",
            name: "Knowledge Jobs",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            "knowledge_email_ingest",
            SCHEMA_EMAIL_INGEST,
        )]
    }

    fn cross_extension_tables(&self) -> Vec<&'static str> {
        vec!["knowledge_documents"]
    }

    fn migrations(&self) -> Vec<Migration> {
        extension_migrations!()
    }

    fn jobs(&self) -> Vec<Arc<dyn Job>> {
        crate::registry::extension_jobs()
    }
}

register_extension!(KnowledgeJobsExtension);
