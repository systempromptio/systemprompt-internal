//! `GET /admin/api/profile/odoo` — the signed-in user's Odoo link state.
//!
//! The profile page renders the same state server-side; this endpoint exists so
//! the page can refresh the card after a link or unlink without a reload.

use axum::{Extension, Json};
use serde::Serialize;

use super::rpc::OdooConnection;
use crate::error::AdminResult;
use crate::handlers::auth_deps::AuthDeps;
use crate::repositories::users::odoo_identity;
use crate::types::UserContext;

#[derive(Debug, Serialize)]
pub(crate) struct IdentityResponse {
    linked: bool,
    configured: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    odoo_login: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    odoo_uid: Option<i32>,
}

pub(crate) async fn odoo_identity_status(
    Extension(user_ctx): Extension<UserContext>,
    Extension(deps): Extension<AuthDeps>,
) -> AdminResult<Json<IdentityResponse>> {
    let identity = odoo_identity::find(&deps.write_pool, &user_ctx.user_id).await?;
    Ok(Json(IdentityResponse {
        linked: identity.is_some(),
        configured: OdooConnection::from_env().is_some(),
        odoo_login: identity.as_ref().map(|i| i.odoo_login.clone()),
        odoo_uid: identity.as_ref().map(|i| i.odoo_uid),
    }))
}
