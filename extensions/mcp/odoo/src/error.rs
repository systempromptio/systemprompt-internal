//! Error type for the `odoo` MCP server.

use axum::http::StatusCode;
use systemprompt::traits::ExtensionError;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum OdooError {
    #[error("Odoo is not configured: {0}")]
    NotConfigured(String),

    #[error("Not linked: {0}")]
    NotLinked(String),

    #[error("Odoo app not installed: {0}")]
    AppMissing(String),

    #[error("Odoo rejected the call: {0}")]
    Odoo(String),

    #[error("Odoo transport error: {0}")]
    Transport(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ExtensionError for OdooError {
    fn code(&self) -> &'static str {
        match self {
            Self::NotConfigured(_) => "NOT_CONFIGURED",
            Self::NotLinked(_) => "NOT_LINKED",
            Self::AppMissing(_) => "APP_NOT_INSTALLED",
            Self::Odoo(_) => "ODOO_REJECTED",
            Self::Transport(_) => "UPSTREAM_UNAVAILABLE",
            Self::Serialization(_) => "SERIALIZATION_ERROR",
            Self::Internal(_) => "INTERNAL_ERROR",
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::NotConfigured(_) | Self::Transport(_) => StatusCode::SERVICE_UNAVAILABLE,
            Self::NotLinked(_) => StatusCode::FORBIDDEN,
            Self::AppMissing(_) => StatusCode::NOT_IMPLEMENTED,
            Self::Odoo(_) => StatusCode::BAD_GATEWAY,
            Self::Serialization(_) | Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    // Why: a transport failure is the only variant where the same call might
    // succeed unchanged; everything else needs the caller, the credential, or
    // the deployment to change first.
    fn is_retryable(&self) -> bool {
        matches!(self, Self::Transport(_))
    }
}

impl From<OdooError> for rmcp::ErrorData {
    fn from(err: OdooError) -> Self {
        match err {
            OdooError::NotLinked(msg)
            | OdooError::NotConfigured(msg)
            | OdooError::AppMissing(msg) => Self::invalid_request(msg, None),
            other => Self::internal_error(other.to_string(), None),
        }
    }
}
