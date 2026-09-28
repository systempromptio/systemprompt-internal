//! Helpers shared across admin handlers.

use std::path::PathBuf;

use axum::http::HeaderMap;
use serde::Serialize;
use systemprompt::config::ProfileBootstrap;

use crate::error::{AdminError, AdminResult};

// Why: built only by `AdminError::into_response`, so a status and its body
// cannot be chosen independently of each other.
#[derive(Debug, Serialize)]
pub(crate) struct ErrorBody {
    pub error: String,
}

pub(crate) fn get_services_path() -> AdminResult<PathBuf> {
    Ok(PathBuf::from(&ProfileBootstrap::get()?.paths.services))
}

// Why: the gateway editor writes the services file the loader reads, never the
// profile — the routes are implementation configuration shipped with the image.
pub(crate) fn get_gateway_file_path() -> AdminResult<PathBuf> {
    Ok(get_services_path()?.join("ai").join("gateway.yaml"))
}

// Why: every browser-driven mutation is same-origin; a cross-site form post
// carries a foreign `Origin` and is refused before any state is read.
pub(crate) fn require_write_origin(headers: &HeaderMap) -> AdminResult<()> {
    let profile = ProfileBootstrap::get().map_err(AdminError::internal)?;
    let expected = url::Url::parse(&profile.server.api_external_url)
        .map_err(AdminError::internal)?
        .origin()
        .ascii_serialization();
    if headers.get("origin").and_then(|v| v.to_str().ok()) != Some(expected.as_str()) {
        return Err(AdminError::Forbidden(
            "Same-origin browser request required".to_owned(),
        ));
    }
    Ok(())
}
