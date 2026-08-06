//! Schema inventory for the knowledge bank.
//!
//! DDL lives in `schema/*.sql` and is embedded with `include_str!`, matching
//! the web extension's convention. Nothing here is SQL text.
//!
//! The registration below is what makes the table appear wherever the host
//! runtime installs extension schemas — `infra db migrate` in a deployment
//! that links this crate, and the integration suite's throwaway database,
//! which discovers registrations from the crates linked into the test binary.
//! The MCP binary additionally calls [`ensure_installed`] at startup, because
//! it is deployed as its own process and cannot assume anything else has run
//! against the tenant database first.

use systemprompt::database::DbPool;
use systemprompt::extension::prelude::{
    Extension, ExtensionMetadata, SchemaDefinition, register_extension,
};

use crate::error::KnowledgeBankError;

pub(crate) const SCHEMA_KNOWLEDGE_DOCUMENTS: &str =
    include_str!("../schema/01_knowledge_documents.sql");

#[doc(hidden)]
#[must_use]
pub fn schema_definitions() -> Vec<SchemaDefinition> {
    vec![SchemaDefinition::new(
        "knowledge_documents",
        SCHEMA_KNOWLEDGE_DOCUMENTS,
    )]
}

/// Schema-only extension: the knowledge bank contributes a table and nothing
/// else — no router, no jobs, no config. It exists so the DDL travels with the
/// crate that queries it.
#[derive(Debug, Default, Clone, Copy)]
pub struct KnowledgeBankExtension;

impl Extension for KnowledgeBankExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "knowledge-bank",
            name: "Knowledge Bank",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        schema_definitions()
    }
}

register_extension!(KnowledgeBankExtension);

/// Apply the DDL if it is not already present.
///
/// Every statement in the file is `IF NOT EXISTS`, so this is idempotent and
/// safe to run on each boot of the MCP process.
///
/// # Errors
/// [`KnowledgeBankError::Internal`] if there is no write pool or the DDL fails.
pub async fn ensure_installed(pool: &DbPool) -> Result<(), KnowledgeBankError> {
    let write = pool.write_pool().ok_or_else(|| {
        KnowledgeBankError::Internal(
            "no Postgres write pool available; the knowledge bank cannot install its schema"
                .to_owned(),
        )
    })?;

    sqlx::raw_sql(SCHEMA_KNOWLEDGE_DOCUMENTS)
        .execute(write.as_ref())
        .await
        .map_err(|e| {
            KnowledgeBankError::Internal(format!("knowledge_documents schema install failed: {e}"))
        })?;

    Ok(())
}
