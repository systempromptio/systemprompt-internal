//! Salesforce token shapes, the authorization-code exchange, banking into the
//! per-user encrypted store, and the typed accessor endpoint core's external-MCP
//! client calls to obtain a fresh Salesforce bearer.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use systemprompt::identifiers::UserId;

use super::config::SalesforceConfig;
use super::{SalesforceDeps, SalesforceError, PLUGIN_ID};
use crate::handlers::users::extract_user_from_cookie;
use crate::repositories::plugins_grp::plugin_env::upsert_plugin_env_var;
use crate::services::salesforce_token;

/// The Salesforce `/services/oauth2/token` response. Salesforce returns
/// `instance_url`/`issued_at` (epoch-ms string) rather than `expires_in`; the
/// refresh-token grant omits `refresh_token` (the original stays valid).
#[derive(Debug, Deserialize)]
pub struct SalesforceTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub refresh_token: Option<String>,
    #[serde(default)]
    pub instance_url: Option<String>,
    #[serde(default)]
    pub issued_at: Option<String>,
}

/// Exchange an authorization code for the full token set.
pub(super) async fn exchange_code(
    cfg: &SalesforceConfig,
    code: &str,
    client_secret: &str,
    code_verifier: &str,
) -> Result<SalesforceTokenResponse, SalesforceError> {
    // reqwest is built with `default-features = false`, so `.form()` is
    // unavailable — encode the body by hand.
    let body = format!(
        "grant_type=authorization_code&code={}&client_id={}&client_secret={}&redirect_uri={}&code_verifier={}",
        urlencoding::encode(code),
        urlencoding::encode(&cfg.client_id),
        urlencoding::encode(client_secret),
        urlencoding::encode(&cfg.redirect_uri),
        urlencoding::encode(code_verifier),
    );
    post_token_request(&cfg.token_url(), body).await
}

/// Shared `application/x-www-form-urlencoded` POST against a Salesforce token
/// endpoint, used by both the code exchange and the refresh service.
pub async fn post_token_request(
    token_url: &str,
    body: String,
) -> Result<SalesforceTokenResponse, SalesforceError> {
    let resp = reqwest::Client::new()
        .post(token_url)
        .header(
            reqwest::header::CONTENT_TYPE,
            "application/x-www-form-urlencoded",
        )
        .body(body)
        .send()
        .await?;
    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        return Err(SalesforceError::TokenEndpoint { status, body });
    }
    Ok(resp.json().await?)
}

/// Bank the freshly-issued tokens for `user_id` under plugin `salesforce`.
///
/// The OAuth secrets land ChaCha20Poly1305-encrypted under the user's DEK;
/// `client_id` is stored in the clear so the refresh service is self-contained
/// (no SSO config needed on the secret-resolution path).
pub(super) async fn bank_tokens(
    pool: &sqlx::PgPool,
    user_id: &UserId,
    tokens: &SalesforceTokenResponse,
    client_id: &str,
) -> Result<(), SalesforceError> {
    upsert_plugin_env_var(
        pool,
        user_id,
        PLUGIN_ID,
        "access_token",
        &tokens.access_token,
        true,
    )
    .await?;
    if let Some(refresh) = tokens.refresh_token.as_deref() {
        upsert_plugin_env_var(pool, user_id, PLUGIN_ID, "refresh_token", refresh, true).await?;
    }
    if let Some(instance) = tokens.instance_url.as_deref() {
        upsert_plugin_env_var(pool, user_id, PLUGIN_ID, "instance_url", instance, true).await?;
    }
    if let Some(issued) = tokens.issued_at.as_deref() {
        upsert_plugin_env_var(pool, user_id, PLUGIN_ID, "issued_at", issued, true).await?;
    }
    upsert_plugin_env_var(pool, user_id, PLUGIN_ID, "client_id", client_id, false).await?;
    Ok(())
}

/// The accessor response: a Salesforce bearer + the instance it is scoped to.
#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    instance_url: String,
}

/// `GET /api/public/salesforce/token` — the typed contract core's Salesforce-MCP
/// bearer injection consumes. Authenticates the caller, refreshes the banked
/// token if stale, and returns `{ access_token, instance_url }`.
pub async fn salesforce_token_handler(
    Extension(deps): Extension<SalesforceDeps>,
    headers: HeaderMap,
) -> Response {
    let Ok(session) = extract_user_from_cookie(&headers) else {
        return (StatusCode::UNAUTHORIZED, "authentication required").into_response();
    };

    match salesforce_token::get_fresh_token(&deps.write_pool, &session.user_id, false).await {
        Ok(fresh) => Json(TokenResponse {
            access_token: fresh.access_token,
            instance_url: fresh.instance_url,
        })
        .into_response(),
        Err(SalesforceError::NoStoredTokens) => {
            (StatusCode::NOT_FOUND, "no Salesforce tokens banked").into_response()
        }
        Err(e) => {
            tracing::error!(error = %e, user_id = %session.user_id, "Salesforce token accessor failed");
            (StatusCode::BAD_GATEWAY, "Salesforce token refresh failed").into_response()
        }
    }
}
