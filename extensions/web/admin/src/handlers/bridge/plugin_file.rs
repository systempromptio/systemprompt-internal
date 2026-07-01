//! Bridge plugin-file endpoint (`GET /v1/bridge/plugins/{id}/{*path}`).
//!
//! Bytes are assembled live from the same `build_plugin_bundle` pipeline the
//! gateway hashes into the signed manifest, so every file the bridge fetches is
//! byte-identical to its manifest hash. Serving the pre-generated
//! `storage/files/plugins/` tree instead drifts from that hash and fails the
//! bridge's manifest verification.

use std::collections::BTreeMap;
use std::path::{Component, Path};

use axum::{
    body::Body,
    extract::Path as AxumPath,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::Response,
};
use systemprompt::config::ProfileBootstrap;
use systemprompt::loader::ConfigLoader;
use systemprompt::marketplace::{plugin_bundles, CatalogContent, PluginBundle};
use systemprompt::models::bridge::ids::PluginId;
use systemprompt::models::AppPaths;

use crate::handlers::shared;

pub async fn handle(
    AxumPath((plugin_id, relative_path)): AxumPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let user_id = match super::validate_bridge_jwt(&headers) {
        Ok(id) => id,
        Err(r) => return *r,
    };

    if !relative_path_is_safe(&relative_path) {
        tracing::warn!(
            user_id = %user_id.as_str(),
            plugin_id = %plugin_id,
            path = %relative_path,
            "rejected non-canonical plugin file path",
        );
        return shared::error_response(StatusCode::BAD_REQUEST, "Invalid path");
    }

    let Ok(id) = PluginId::try_new(&plugin_id) else {
        return shared::error_response(StatusCode::NOT_FOUND, "Plugin not found");
    };

    let bundles = match build_bundles() {
        Ok(b) => b,
        Err(r) => return *r,
    };

    let Some(bundle) = bundles.get(&id) else {
        return shared::error_response(
            StatusCode::NOT_FOUND,
            &format!("Plugin '{plugin_id}' not found"),
        );
    };
    let Some(file) = bundle.get(relative_path.as_str()) else {
        return shared::error_response(StatusCode::NOT_FOUND, "File not found");
    };

    build_file_response(&relative_path, file.bytes.clone())
}

fn build_bundles() -> Result<BTreeMap<PluginId, PluginBundle>, Box<Response>> {
    let internal = |stage: &'static str, e: &dyn std::fmt::Display| -> Box<Response> {
        tracing::error!(error = %e, stage, "plugin bundle assembly failed");
        Box::new(shared::error_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Plugin bundle unavailable",
        ))
    };

    let services = ConfigLoader::load().map_err(|e| internal("config", &e))?;
    let profile = ProfileBootstrap::get().map_err(|e| internal("profile", &e))?;
    let paths = AppPaths::from_profile(&profile.paths).map_err(|e| internal("paths", &e))?;

    let catalog = CatalogContent::load(
        &services,
        paths.system().services(),
        &profile.server.api_external_url,
    )
    .map_err(|e| internal("catalog", &e))?;
    plugin_bundles(&services, &catalog.as_content()).map_err(|e| internal("bundle", &e))
}

fn relative_path_is_safe(relative: &str) -> bool {
    !relative.is_empty()
        && Path::new(relative)
            .components()
            .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
}

fn build_file_response(relative_path: &str, bytes: Vec<u8>) -> Response {
    let mut response = Response::new(Body::from(bytes));
    *response.status_mut() = StatusCode::OK;
    if let Ok(value) = HeaderValue::from_str(content_type(relative_path)) {
        response.headers_mut().insert(header::CONTENT_TYPE, value);
    }
    response
}

fn content_type(relative_path: &str) -> &'static str {
    let ext = Path::new(relative_path)
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("md") => "text/markdown; charset=utf-8",
        Some("txt") => "text/plain; charset=utf-8",
        Some("json") => "application/json",
        Some("yaml" | "yml") => "application/yaml",
        Some("toml") => "application/toml",
        Some("html" | "htm") => "text/html; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "application/javascript",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("svg") => "image/svg+xml",
        Some("wasm") => "application/wasm",
        _ => "application/octet-stream",
    }
}
