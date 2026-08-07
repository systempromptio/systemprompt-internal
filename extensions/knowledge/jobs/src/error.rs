//! Error type shared by the knowledge jobs.

use systemprompt::traits::ProviderError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum KnowledgeJobError {
    #[error("Job context missing required value: {0}")]
    MissingContext(&'static str),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IMAP error: {0}")]
    Imap(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("{0}")]
    Other(String),
}

impl From<KnowledgeJobError> for ProviderError {
    fn from(err: KnowledgeJobError) -> Self {
        Self::Internal(err.to_string())
    }
}
