//! `GET /admin/auth/salesforce/callback` — exchange the code for tokens, gate
//! on verified email + allow-listed domain, resolve the identity to a local
//! user, and set the session. The Salesforce Hosted-MCP bearer is no longer
//! banked here; it is minted on demand via the JWT-bearer grant at access time.

use axum::Extension;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use systemprompt::identifiers::SessionSource;
use systemprompt::models::Config;
use systemprompt::models::auth::{AuthenticatedUser, Permission};
use systemprompt::oauth::SessionCreationService;
use systemprompt::oauth::services::{
    JwtConfig, JwtSigningParams, generate_access_token_jti, generate_jwt,
};

use super::identity::resolve_identity;
use super::{STATE_COOKIE, SalesforceDeps, login_error, read_state_cookie, secure_flag};
use crate::repositories::users_grp::federated;

#[derive(Deserialize)]
pub(crate) struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

/// A completed Salesforce login, ready to be turned into a cookie-setting
/// redirect.
struct SuccessfulLogin {
    redirect_to: String,
    jwt: String,
    max_age: i64,
}

pub(crate) async fn salesforce_callback(
    Extension(deps): Extension<SalesforceDeps>,
    headers: HeaderMap,
    Query(params): Query<CallbackParams>,
) -> Response {
    if !deps.config.is_usable() {
        return login_error("unavailable");
    }
    match run_callback(&deps, &headers, params).await {
        Ok(login) => success_response(&login),
        Err(reason) => login_error(reason),
    }
}

/// Drive the callback end-to-end, returning a short error *reason* (surfaced as
/// `?sso=<reason>` on the login page) on any failure.
async fn run_callback(
    deps: &SalesforceDeps,
    headers: &HeaderMap,
    params: CallbackParams,
) -> Result<SuccessfulLogin, &'static str> {
    let (code, code_verifier, redirect_to) = validate_request(headers, params)?;

    let resolved = resolve_identity(deps, &code, &code_verifier).await?;

    let (jwt, max_age) = mint_session(&deps.session_service, &resolved, headers)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to mint session for Salesforce user");
            "error"
        })?;

    tracing::info!(user_id = %resolved.user_id, email = %resolved.email, "Salesforce SSO login succeeded");

    Ok(SuccessfulLogin {
        redirect_to,
        jwt,
        max_age,
    })
}

/// Validate the OAuth callback shape: surface a provider error, require
/// `code`/`state`, check the CSRF state against the cookie, and recover the
/// PKCE verifier + post-login redirect target.
fn validate_request(
    headers: &HeaderMap,
    params: CallbackParams,
) -> Result<(String, String, String), &'static str> {
    if let Some(err) = params.error {
        tracing::warn!(error = %err, detail = ?params.error_description, "Salesforce returned an OAuth error");
        return Err("denied");
    }

    let (Some(code), Some(state)) = (params.code, params.state) else {
        return Err("error");
    };

    let (cookie_state, code_verifier, redirect_to) = read_state_cookie(headers).ok_or("error")?;
    if cookie_state != state {
        tracing::warn!("Salesforce OAuth state mismatch");
        return Err("error");
    }
    Ok((code, code_verifier, redirect_to))
}

/// Build the cookie-setting redirect for a successful login: clear the spent
/// state cookie and set the session `access_token`.
fn success_response(login: &SuccessfulLogin) -> Response {
    let mut out = HeaderMap::new();
    if let Ok(val) = format!(
        "{STATE_COOKIE}=; Path=/admin/auth/salesforce; HttpOnly; SameSite=Lax; Max-Age=0{}",
        secure_flag()
    )
    .parse()
    {
        out.append(SET_COOKIE, val);
    }
    if let Ok(val) = format!(
        "access_token={}; Path=/; HttpOnly; SameSite=Lax; Max-Age={}{}",
        login.jwt,
        login.max_age,
        secure_flag()
    )
    .parse()
    {
        out.append(SET_COOKIE, val);
    }
    (out, Redirect::to(&login.redirect_to)).into_response()
}

/// Mint a systemprompt session JWT for a resolved user, returning the signed
/// token and its max-age (seconds). Mirrors core's OAuth `/token` minting.
async fn mint_session(
    session_service: &SessionCreationService,
    resolved: &federated::ResolvedFederatedUser,
    headers: &HeaderMap,
) -> Result<(String, i64), String> {
    let session_id = session_service
        .create_authenticated_session(&resolved.user_id, headers, SessionSource::Oauth)
        .await
        .map_err(|e| e.to_string())?;

    let cfg = Config::get().map_err(|e| e.to_string())?;
    let uuid = resolved
        .user_id
        .as_str()
        .parse()
        .unwrap_or_else(|_| uuid::Uuid::nil());
    let user = AuthenticatedUser::new_with_roles(
        uuid,
        resolved.display_name.clone(),
        resolved.email.clone(),
        vec![Permission::User],
        resolved.roles.clone(),
    );

    let jwt_config = JwtConfig {
        permissions: vec![Permission::User],
        audience: cfg.jwt_audiences.clone(),
        expires_in_hours: Some(cfg.jwt_access_token_expiration / 3600),
        resource: None,
        plugin_id: None,
    };
    let signing = JwtSigningParams {
        issuer: &cfg.jwt_issuer,
    };
    let jti = generate_access_token_jti();
    let token =
        generate_jwt(&user, jwt_config, jti, &session_id, &signing).map_err(|e| e.to_string())?;

    Ok((token, cfg.jwt_access_token_expiration))
}
