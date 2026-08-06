//! `POST /admin/api/profile/odoo/unlink` — detach the signed-in user's Odoo
//! credential.

use axum::{Extension, Json};
use serde::Serialize;

use crate::error::AdminResult;
use crate::handlers::auth_deps::AuthDeps;
use crate::repositories::users::odoo_identity;
use crate::types::UserContext;

#[derive(Debug, Serialize)]
pub(crate) struct UnlinkResponse {
    unlinked: bool,
}

// Why: unlinking Odoo cannot lock anyone out — it is not a sign-in method, only
// a CRM credential — so unlike the old Salesforce disconnect there is no
// passkey precondition here. It does stop every odoo tool call for this user,
// which the profile page says before it posts.
pub(crate) async fn odoo_unlink(
    Extension(user_ctx): Extension<UserContext>,
    Extension(deps): Extension<AuthDeps>,
) -> AdminResult<Json<UnlinkResponse>> {
    odoo_identity::delete(&deps.write_pool, &user_ctx.user_id).await?;
    tracing::info!(user_id = %user_ctx.user_id, "Odoo identity unlinked");
    Ok(Json(UnlinkResponse { unlinked: true }))
}
