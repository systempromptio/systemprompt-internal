//! `POST /admin/auth/odoo/login` — sign in with an Odoo account.
//!
//! Odoo is the identity provider for this deployment. Odoo Community exposes no
//! OAuth provider, so the proof is a `common.authenticate` JSON-RPC call: if
//! Odoo returns a uid for the submitted login and secret, the caller controls
//! that Odoo account. `authenticate` accepts a password or a personal API key
//! in the same argument, which is why one field serves both — users with 2FA
//! must use an API key, because Odoo refuses their password over RPC.
//!
//! A successful call is turned into a browser session the same way the passkey
//! ceremony is: mint an OAuth authorization code bound to the request's PKCE
//! challenge and let the client exchange it at the token endpoint. That keeps
//! one session-issuing path in the system rather than a second, bespoke one.
//!
//! A successful sign-in also links the user's `odoo_identity` row with the
//! credential that just authenticated, sealed under the master key exactly as
//! the profile-page link flow seals API keys. This is deliberate: whatever
//! worked over RPC here keeps working for the Odoo MCP server, and users with
//! 2FA can only sign in with an API key in the first place, so the stored
//! secret is a password only when Odoo itself accepts that password over RPC.
//! The caveat is rotation: changing the Odoo password or revoking the API key
//! strands the stored copy until the next sign-in (or a profile re-link)
//! refreshes it. The link is best-effort — a failure to store it never blocks
//! the sign-in, it only means the profile still shows Odoo as unconnected.

use axum::http::HeaderMap;
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};
use systemprompt::identifiers::{AuthorizationCode, ClientId, UserId};
use systemprompt::oauth::OAuthRepository;
use systemprompt::oauth::repository::AuthCodeParams;
use systemprompt::oauth::services::generate_secure_token;

use super::OdooAuthError;
use super::rpc::{OdooConnection, authenticate};
use crate::error::{AdminError, AdminResult};
use crate::handlers::auth_deps::AuthDeps;
use crate::repositories::users::federated::{FederatedClaims, resolve_federated_user};
use crate::repositories::users::odoo_identity;

// Why: the issuer for `federated_identities.issuer` is the base URL, not the
// bare string "odoo" — a deployment repointed at another Odoo is a different
// identity namespace, and uid 2 there is not uid 2 here.
fn issuer_for(conn: &OdooConnection) -> String {
    format!("odoo:{}/{}", conn.url, conn.db)
}

#[derive(Debug, Deserialize)]
pub(crate) struct OdooLoginRequest {
    login: String,
    credential: String,
    client_id: ClientId,
    redirect_uri: String,
    code_challenge: String,
    code_challenge_method: String,
    #[serde(default)]
    scope: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct OdooLoginResponse {
    authorization_code: String,
    redirect_uri: String,
    state: Option<String>,
}

pub(crate) async fn odoo_login(
    Extension(deps): Extension<AuthDeps>,
    headers: HeaderMap,
    Json(req): Json<OdooLoginRequest>,
) -> AdminResult<Json<OdooLoginResponse>> {
    let login = req.login.trim().to_lowercase();
    let credential = req.credential.trim().to_owned();
    validate_request(&req, &login, &credential)?;

    let client_key = client_key(&headers);
    for key in [login.as_str(), client_key.as_str()] {
        if deps.login_throttle.is_blocked(key) {
            tracing::warn!(login, "Odoo sign-in throttled");
            return Err(AdminError::RateLimited(
                "Too many sign-in attempts. Wait fifteen minutes and try again.".to_owned(),
            ));
        }
    }

    let conn = OdooConnection::from_env().ok_or_else(|| {
        OdooAuthError::NotConfigured(
            "ODOO_URL and ODOO_DB are not set on this server; ask your administrator to \
             configure the Odoo connection."
                .to_owned(),
        )
    })?;

    let uid = match authenticate(&conn, &login, &credential).await {
        Ok(Some(uid)) => uid,
        Ok(None) => {
            deps.login_throttle.record_failure(&login);
            deps.login_throttle.record_failure(&client_key);
            tracing::warn!(login, "Odoo rejected sign-in credential");
            return Err(OdooAuthError::InvalidCredential.into());
        },
        Err(e) => {
            tracing::error!(error = %e, login, "Odoo authenticate call failed");
            return Err(OdooAuthError::Rpc(e).into());
        },
    };

    // Why: `auto_provision` is on. Anyone who can authenticate against this
    // Odoo gets a platform account on first sign-in — the deliberate choice
    // that makes an Odoo admin a platform user without an operator step. It
    // also means Odoo's own user list is the access boundary here.
    let resolved = resolve_federated_user(
        &deps.write_pool,
        &FederatedClaims {
            issuer: &issuer_for(&conn),
            external_sub: &uid.to_string(),
            email: &login,
            display_name: &login,
        },
        true,
    )
    .await
    .map_err(AdminError::Marketplace)?
    .ok_or_else(|| {
        AdminError::Forbidden(
            "No account exists for this Odoo user, and one could not be created.".to_owned(),
        )
    })?;

    auto_link_identity(
        &deps.write_pool,
        &resolved.user_id,
        &login,
        uid,
        &credential,
    )
    .await;

    let code = mint_authorization_code(&deps.oauth_repo, &req, &resolved.user_id).await?;

    deps.login_throttle.record_success(&login);
    deps.login_throttle.record_success(&client_key);
    tracing::info!(
        user_id = %resolved.user_id,
        login,
        odoo_uid = uid,
        "Odoo sign-in succeeded"
    );

    Ok(Json(OdooLoginResponse {
        authorization_code: code,
        redirect_uri: req.redirect_uri.clone(),
        state: req.state.clone(),
    }))
}

fn validate_request(req: &OdooLoginRequest, login: &str, credential: &str) -> AdminResult<()> {
    if login.is_empty() || credential.is_empty() {
        return Err(AdminError::BadRequest(
            "An Odoo login and password or API key are both required".to_owned(),
        ));
    }
    // Why: resolution keys on email, and every downstream consumer (seat
    // accounting, org membership, the profile page) assumes users.email is a
    // real address. An Odoo login like "admin" has nothing to key on.
    if !login.contains('@') {
        return Err(AdminError::BadRequest(
            "Sign in with the email address on your Odoo account".to_owned(),
        ));
    }
    if req.code_challenge.trim().is_empty() || req.code_challenge_method.trim().is_empty() {
        return Err(AdminError::BadRequest(
            "A PKCE code challenge is required".to_owned(),
        ));
    }
    Ok(())
}

// Why: the credential just proved itself over RPC, so auto-linking it keeps
// Odoo MCP tools working without a profile-page step (see the module doc).
// Best-effort: sign-in must not fail because the link write did.
async fn auto_link_identity(
    pool: &sqlx::PgPool,
    user_id: &UserId,
    login: &str,
    uid: i32,
    credential: &str,
) {
    if let Err(e) = odoo_identity::insert(pool, user_id, login, uid, credential).await {
        tracing::error!(
            error = %e,
            user_id = %user_id,
            login,
            "Odoo sign-in succeeded but auto-linking the Odoo identity failed; \
             the user can still link from their profile"
        );
    }
}

// Why: mirrors the code issuance in core's `webauthn_complete`, so both
// sign-in routes produce codes the token endpoint treats identically.
async fn mint_authorization_code(
    repo: &OAuthRepository,
    req: &OdooLoginRequest,
    user_id: &UserId,
) -> AdminResult<String> {
    let code_str = generate_secure_token("auth_code");
    let code = AuthorizationCode::new(code_str.clone());

    let scope = req.scope.clone().unwrap_or_else(|| {
        let default_roles = OAuthRepository::get_default_roles();
        if default_roles.is_empty() {
            "user".to_owned()
        } else {
            default_roles.join(" ")
        }
    });

    let params = AuthCodeParams::builder(&code, &req.client_id, user_id, &req.redirect_uri, &scope)
        .with_pkce(&req.code_challenge, &req.code_challenge_method)
        .build();

    repo.store_authorization_code(params)
        .await
        .map_err(AdminError::internal)?;

    Ok(code_str)
}

// Why: best-effort caller identity for throttling. The server sits behind a
// proxy, so the socket address is the proxy's. Fall back to a single shared
// bucket when no forwarding header is present: that throttles all header-less
// callers together, which is the safe direction to fail.
fn client_key(headers: &HeaderMap) -> String {
    let forwarded = headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(',').next())
        .map(str::trim)
        .filter(|v| !v.is_empty());

    let real_ip = headers
        .get("x-real-ip")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty());

    format!("ip:{}", forwarded.or(real_ip).unwrap_or("unknown"))
}
