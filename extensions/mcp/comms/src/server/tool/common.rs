//! Shared helpers for the comms tool handlers.

use rmcp::ErrorData as McpError;
use systemprompt::models::artifacts::{CliArtifact, TextArtifact};

pub(super) fn text_artifact(title: &str, body: &str) -> CliArtifact {
    CliArtifact::text(TextArtifact::new(body).with_title(title))
}

pub(super) fn invalid(e: impl std::fmt::Display) -> McpError {
    McpError::invalid_params(e.to_string(), None)
}

pub(super) fn internal(e: impl std::fmt::Display) -> McpError {
    McpError::internal_error(e.to_string(), None)
}
