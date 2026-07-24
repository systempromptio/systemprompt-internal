//! Error type shared by the access-token repositories.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum AccessTokenRepoError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Validation error: {0}")]
    Validation(String),
}

pub type Result<T> = std::result::Result<T, AccessTokenRepoError>;
