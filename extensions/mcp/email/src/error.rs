//! Errors from the email MCP server.

use rmcp::ErrorData as McpError;

#[derive(Debug, thiserror::Error)]
pub enum EmailToolError {
    #[error("{0}")]
    Invalid(String),

    #[error("{0}")]
    Transport(#[from] systemprompt_email::EmailError),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl From<EmailToolError> for McpError {
    fn from(err: EmailToolError) -> Self {
        match err {
            EmailToolError::Invalid(message) => Self::invalid_params(message, None),
            other => Self::internal_error(other.to_string(), None),
        }
    }
}
