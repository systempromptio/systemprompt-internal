//! `POST /admin/api/profile/odoo/link` — attach the signed-in user's Odoo
//! credential.

use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use super::rpc::{OdooConnection, authenticate};
use super::OdooAuthError;
use crate::error::{AdminError, AdminResult};
use crate::handlers::auth_deps::AuthDeps;
use crate::repositories::users::odoo_identity;
use crate::types::UserContext;

#[derive(Debug, Deserialize)]
pub(crate) struct LinkRequest {
    login: String,
    api_key: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct LinkResponse {
    linked: bool,
    odoo_login: String,
    odoo_uid: i32,
}

// Why: the credential is proven against Odoo before it is stored. An
// unverified key moves the failure to the first tool call, where the user is
// not at the form and the error surfaces as an agent malfunction.
pub(crate) async fn odoo_link(
    Extension(user_ctx): Extension<UserContext>,
    Extension(deps): Extension<AuthDeps>,
    Json(req): Json<LinkRequest>,
) -> AdminResult<Json<LinkResponse>> {
    let login = req.login.trim().to_owned();
    let api_key = req.api_key.trim().to_owned();
    if login.is_empty() || api_key.is_empty() {
        return Err(AdminError::BadRequest(
            "Both an Odoo login and an API key are required".to_owned(),
        ));
    }

    let conn = OdooConnection::from_env().ok_or_else(|| {
        OdooAuthError::NotConfigured(
            "ODOO_URL and ODOO_DB are not set on this server; ask your administrator to \
             configure the Odoo connection."
                .to_owned(),
        )
    })?;

    let uid = authenticate(&conn, &login, &api_key)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, user_id = %user_ctx.user_id, "Odoo authenticate call failed");
            OdooAuthError::Rpc(e)
        })?
        .ok_or(OdooAuthError::InvalidCredential)?;

    odoo_identity::insert(&deps.write_pool, &user_ctx.user_id, &login, uid, &api_key)
        .await
        .map_err(OdooAuthError::Storage)?;

    tracing::info!(user_id = %user_ctx.user_id, odoo_uid = uid, "Odoo account linked");

    Ok(Json(LinkResponse {
        linked: true,
        odoo_login: login,
        odoo_uid: uid,
    }))
}
