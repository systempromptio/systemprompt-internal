//! Schema inventory for team comms.
//!
//! DDL lives in `schema/*.sql` and is embedded with `include_str!`. The MCP
//! binary calls [`ensure_installed`] at startup because it is deployed as its
//! own process and cannot assume anything else has run against the tenant
//! database first.

use systemprompt::database::DbPool;
use systemprompt::extension::prelude::{
    Extension, ExtensionMetadata, SchemaDefinition, register_extension,
};

use crate::error::CommsError;

pub(crate) const SCHEMA_COMMS: &str = include_str!("../schema/01_comms.sql");

#[doc(hidden)]
#[must_use]
pub fn schema_definitions() -> Vec<SchemaDefinition> {
    vec![SchemaDefinition::new("comms_messages", SCHEMA_COMMS)]
}

#[derive(Debug, Default, Clone, Copy)]
pub struct CommsExtension;

impl Extension for CommsExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: "comms",
            name: "Team Comms",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        schema_definitions()
    }
}

register_extension!(CommsExtension);

pub async fn ensure_installed(pool: &DbPool) -> Result<(), CommsError> {
    let write = pool.write_pool().ok_or_else(|| {
        CommsError::Internal(
            "no Postgres write pool available; comms cannot install its schema".to_owned(),
        )
    })?;

    sqlx::raw_sql(SCHEMA_COMMS)
        .execute(write.as_ref())
        .await
        .map_err(|e| CommsError::Internal(format!("comms schema install failed: {e}")))?;

    Ok(())
}
