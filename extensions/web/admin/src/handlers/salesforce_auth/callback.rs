//! `GET /admin/auth/salesforce/callback` — exchange the code for tokens, gate on
//! verified email + allow-listed domain, resolve the identity to a local user,
//! and set the session. The Salesforce Hosted-MCP bearer is no longer banked
//! here; it is minted on demand via the JWT-bearer grant at access time.

use axum::extract::Query;
use axum::http::header::SET_COOKIE;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Extension;
use serde::Deserialize;

use systemprompt::identifiers::SessionSource;
use systemprompt::models::auth::{AuthenticatedUser, Permission};
use systemprompt::models::Config;
use systemprompt::oauth::services::{
    generate_access_token_jti, generate_jwt, JwtConfig, JwtSigningParams,
};
use systemprompt::oauth::SessionCreationService;

use super::config::SalesforceConfig;
use super::tokens::exchange_code;
use super::{login_error, read_state_cookie, secure_flag, SalesforceDeps, STATE_COOKIE};
use crate::repositories::users_grp::{federated, salesforce_identity};

#[derive(Deserialize)]
pub struct CallbackParams {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Deserialize)]
struct SalesforceUserInfo {
    sub: String,
    email: Option<String>,
    #[serde(default)]
    email_verified: bool,
    name: Option<String>,
    /// The Salesforce Username (e.g. `ed.aa…@agentforce.com`) — distinct from the
    /// login email, and the value the JWT-bearer grant matches on `sub`.
    preferred_username: Option<String>,
}

/// A completed Salesforce login, ready to be turned into a cookie-setting
/// redirect.
struct SuccessfulLogin {
    redirect_to: String,
    jwt: String,
    max_age: i64,
}

pub async fn salesforce_callback(
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
/// `code`/`state`, check the CSRF state against the cookie, and recover the PKCE
/// verifier + post-login redirect target.
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

/// Exchange the code for tokens, read verified claims, gate them, and resolve
/// the identity to a local user. The access token is used only to fetch
/// userinfo here — it is not retained. Each step logs its own failure and
/// collapses to a login *reason*.
async fn resolve_identity(
    deps: &SalesforceDeps,
    code: &str,
    code_verifier: &str,
) -> Result<federated::ResolvedFederatedUser, &'static str> {
    let cfg = &deps.config;

    let client_secret = super::client_secret().ok_or_else(|| {
        tracing::error!("SALESFORCE_CLIENT_SECRET is not set; cannot complete Salesforce login");
        "unavailable"
    })?;

    let tokens = exchange_code(cfg, code, &client_secret, code_verifier)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Salesforce token exchange failed");
            "error"
        })?;

    let info = fetch_userinfo(cfg, &tokens.access_token)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Salesforce userinfo fetch failed");
            "error"
        })?;

    let (sub, email, display_name, sf_username) = gate_claims(cfg, info)?;

    let claims = federated::FederatedClaims {
        issuer: cfg.issuer(),
        external_sub: &sub,
        email: &email,
        display_name: &display_name,
    };
    let resolved = federated::resolve_federated_user(&deps.write_pool, &claims, cfg.auto_provision)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to resolve federated Salesforce user");
            "error"
        })?
        .ok_or_else(|| {
            tracing::warn!(
                email,
                "Salesforce login rejected: auto-provisioning disabled and no existing account"
            );
            "not_provisioned"
        })?;

    // Record the Salesforce Username so the Hosted-MCP token accessor can mint a
    // JWT-bearer token as this user. A failure here must not break login — the
    // accessor falls back to the email if no row exists.
    if let Err(e) =
        salesforce_identity::upsert(&deps.write_pool, resolved.user_id.as_str(), &sf_username).await
    {
        tracing::warn!(error = %e, user_id = %resolved.user_id, "Failed to persist Salesforce username");
    }

    Ok(resolved)
}

/// Enforce the verified-email + allow-listed-domain gate, returning the
/// `(sub, email, display_name, sf_username)` to provision/link with. `sf_username`
/// is the Salesforce Username (userinfo `preferred_username`), falling back to the
/// email when Salesforce omits it.
fn gate_claims(
    cfg: &SalesforceConfig,
    info: SalesforceUserInfo,
) -> Result<(String, String, String, String), &'static str> {
    let email = info
        .email
        .map(|e| e.trim().to_lowercase())
        .ok_or("no_email")?;
    // Linking an unverified address would let a hostile IdP claim arbitrary
    // accounts via the email-merge path in `federated`.
    if !info.email_verified {
        tracing::warn!(email, "Salesforce login rejected: email not verified");
        return Err("unverified");
    }
    if !cfg.email_allowed(&email) {
        tracing::warn!(email, "Salesforce login rejected: domain not allow-listed");
        return Err("forbidden");
    }
    let display_name = info.name.unwrap_or_else(|| email.clone());
    let sf_username = select_sf_username(info.preferred_username.as_deref(), &email);
    Ok((info.sub, email, display_name, sf_username))
}

/// The Salesforce Username to sign JWT-bearer assertions with.
///
/// Uses the userinfo `preferred_username` when present and non-blank, else the
/// login email — the latter is only correct for orgs where the email *is* the
/// Username.
pub fn select_sf_username(preferred_username: Option<&str>, email: &str) -> String {
    preferred_username
        .map(str::trim)
        .filter(|u| !u.is_empty())
        .map_or_else(|| email.to_owned(), str::to_owned)
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

async fn fetch_userinfo(
    cfg: &SalesforceConfig,
    access_token: &str,
) -> Result<SalesforceUserInfo, super::SalesforceError> {
    let resp = reqwest::Client::new()
        .get(cfg.userinfo_url())
        .bearer_auth(access_token)
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(super::SalesforceError::UserInfo(resp.status()));
    }
    Ok(resp.json().await?)
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
