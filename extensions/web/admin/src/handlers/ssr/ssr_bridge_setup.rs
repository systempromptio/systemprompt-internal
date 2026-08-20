//! SSR page walking a user through bridge installation.

use std::sync::Arc;

use axum::extract::{Extension, State};
use axum::http::HeaderMap;
use axum::response::Response;
use serde::Serialize;
use sqlx::PgPool;

use crate::error::AdminHtmlResult;
use crate::repositories::users::odoo_identity;
use crate::templates::AdminTemplateEngine;
use crate::types::{MarketplaceContext, UserContext};

use super::ssr_helpers::render_typed_page;

// Why: the website is the download source of truth — `just deploy` stages the
// binaries into `storage/files/downloads/` (scripts/package-bridge-*.sh) and
// the server serves them same-origin at /files/downloads. Asset names stay in
// lockstep with those scripts, `bridge-setup.hbs`, and `ARTIFACTS` in
// `storage/files/js/pages/admin-bridge-setup.js`.
const DOWNLOAD_BASE_URL: &str = "/files/downloads";

#[derive(Debug, Serialize)]
struct SetupPageData {
    gateway_url: String,
    user_email: String,
    download_base_url: &'static str,
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
    let data = SetupPageData {
        gateway_url: derive_gateway_url(&headers),
        user_email: user_ctx.email.to_string(),
        download_base_url: DOWNLOAD_BASE_URL,
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
