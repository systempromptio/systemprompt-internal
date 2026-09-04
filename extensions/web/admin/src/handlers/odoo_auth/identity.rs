//! `GET /admin/api/profile/odoo` — the signed-in user's Odoo link state.
//!
//! The profile page renders the same state server-side; this endpoint exists so
//! the page can refresh the card after a link or unlink without a reload.

use axum::{Extension, Json};
use serde::Serialize;

use super::rpc::{OdooConnection, authenticate};
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
    // Why: Whether the stored credential authenticates against Odoo right now.
    // `None` when nothing is linked, or when Odoo could not be reached to
    // ask — an unreachable Odoo is not evidence that the credential is bad.
    #[serde(skip_serializing_if = "Option::is_none")]
    credential_live: Option<bool>,
}

pub(crate) async fn odoo_identity_status(
    Extension(user_ctx): Extension<UserContext>,
    Extension(deps): Extension<AuthDeps>,
) -> AdminResult<Json<IdentityResponse>> {
    let identity = odoo_identity::find(&deps.write_pool, &user_ctx.user_id).await?;
    let conn = OdooConnection::from_env();
    let credential_live = match (&identity, &conn) {
        (Some(identity), Some(conn)) => {
            probe_credential(&deps, &user_ctx, identity.odoo_login.as_str(), conn).await
        },
        _ => None,
    };
    Ok(Json(IdentityResponse {
        linked: identity.is_some(),
        configured: conn.is_some(),
        odoo_login: identity.as_ref().map(|i| i.odoo_login.clone()),
        odoo_uid: identity.as_ref().map(|i| i.odoo_uid),
        credential_live,
    }))
}

// Why: a linked account says nothing about whether the credential still works.
// Odoo revokes keys on a password change, and a restored database drops them
// outright — both leave a row that looks healthy and fails at the first tool
// call, somewhere the user cannot act on it. One `authenticate` here moves
// that discovery to the page that carries the relink control.
//
// Returns None rather than Some(false) when the probe itself could not run:
// "we could not ask Odoo" and "Odoo said no" are different answers, and
// reporting the first as the second sends the user to replace a working key.
async fn probe_credential(
    deps: &AuthDeps,
    user_ctx: &UserContext,
    login: &str,
    conn: &OdooConnection,
) -> Option<bool> {
    let api_key = match odoo_identity::find_api_key(&deps.write_pool, &user_ctx.user_id).await {
        Ok(Some(key)) => key,
        Ok(None) => return None,
        Err(e) => {
            tracing::warn!(error = %e, user_id = %user_ctx.user_id, "Could not open the stored Odoo credential to probe it");
            return None;
        },
    };
    match authenticate(conn, login, &api_key).await {
        Ok(uid) => Some(uid.is_some()),
        Err(e) => {
            tracing::warn!(error = %e, user_id = %user_ctx.user_id, "Odoo credential probe could not reach Odoo");
            None
        },
    }
}
