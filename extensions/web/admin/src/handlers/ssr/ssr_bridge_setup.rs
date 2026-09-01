//! SSR page walking a user through bridge installation.

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde::Serialize;
use sqlx::PgPool;

use crate::error::AdminHtmlResult;
use crate::repositories::users::odoo_identity;
use crate::services::bridge_downloads;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

use super::ssr_helpers::render_typed_page;

#[derive(Debug, Serialize)]
struct SetupPageData {
    gateway_url: String,
    user_email: String,
    download_base_url: String,
    release_page_url: String,
    bridge_version: &'static str,
    install_command: String,
    asset_macos: String,
    asset_windows: String,
    asset_linux_x86_64: String,
    asset_linux_aarch64: String,
    odoo_linked: bool,
}

pub(crate) async fn bridge_setup_page(
    State(pool): State<Arc<PgPool>>,
    Extension(user_ctx): Extension<UserContext>,
    Extension(mkt_ctx): Extension<MarketplaceContext>,
    Extension(engine): Extension<AdminTemplateEngine>,
    headers: HeaderMap,
) -> AdminHtmlResult<Response> {
    let odoo_linked = odoo_identity::find(&pool, &user_ctx.user_id)
        .await?
        .is_some();
    let gateway_url = derive_gateway_url(&headers);
    let data = SetupPageData {
        install_command: bridge_downloads::install_command(&gateway_url, None),
        gateway_url,
        user_email: user_ctx.email.to_string(),
        download_base_url: bridge_downloads::release_base_url(),
        release_page_url: bridge_downloads::release_page_url(),
        bridge_version: bridge_downloads::BRIDGE_VERSION,
        asset_macos: bridge_downloads::asset_name("macos.dmg"),
        asset_windows: bridge_downloads::asset_name("windows.exe"),
        asset_linux_x86_64: bridge_downloads::asset_name("linux-x86_64.tar.gz"),
        asset_linux_aarch64: bridge_downloads::asset_name("linux-aarch64.tar.gz"),
        odoo_linked,
    };
    Ok(render_typed_page(
        &engine,
        "bridge-setup",
        &data,
        &user_ctx,
        &mkt_ctx,
    ))
}

fn derive_gateway_url(headers: &HeaderMap) -> String {
    let scheme = headers
        .get("x-forwarded-proto")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("http");
    let host = headers
        .get("host")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("localhost:8080");
    format!("{scheme}://{host}")
}
