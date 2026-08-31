//! Errors this server returns.

use rmcp::ErrorData as McpError;
use systemprompt_factsheet::FactsheetError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ServerError {
    #[error(transparent)]
    Engine(#[from] FactsheetError),

    #[error("Could not store the rendered factsheet: {0}")]
    Storage(String),

    #[error("{0}")]
    Internal(String),
}

impl From<ServerError> for McpError {
    fn from(error: ServerError) -> Self {
        match &error {
            // A bad sheet id or an overlong sheet is the caller's to fix, and
            // the message says how — so it must reach them as invalid params
            // rather than as an opaque internal error.
            ServerError::Engine(
                FactsheetError::SheetMissing(_)
                | FactsheetError::PageBudget { .. }
                | FactsheetError::Parse { .. },
            ) => Self::invalid_params(error.to_string(), None),
            _ => Self::internal_error(error.to_string(), None),
        }
    }
}

pub type ServerResult<T> = Result<T, ServerError>;
