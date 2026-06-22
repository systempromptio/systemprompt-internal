//! Salesforce token shapes, the authorization-code exchange (used by the SSO
//! login callback), and the typed accessor endpoint core's external-MCP client
//! calls to obtain a fresh Salesforce bearer. The accessor no longer reads a
//! banked token — it mints one on demand via the RFC 7523 JWT-bearer grant.

use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use serde::{Deserialize, Serialize};

use super::config::SalesforceConfig;
use super::{SalesforceDeps, SalesforceError};
use crate::handlers::users::extract_user_from_cookie;
use crate::services::salesforce_jwt_bearer;

/// The Salesforce `/services/oauth2/token` response. Only the fields both the
/// authorization-code (login) and JWT-bearer (Hosted-MCP) flows consume are
/// modelled; other fields are ignored.
#[derive(Debug, Deserialize)]
pub struct SalesforceTokenResponse {
    pub access_token: String,
    #[serde(default)]
    pub instance_url: Option<String>,
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

/// The accessor response: a Salesforce bearer + the instance it is scoped to.
#[derive(Debug, Serialize)]
struct TokenResponse {
    access_token: String,
    instance_url: String,
}

/// `GET /api/public/salesforce/token` — the typed contract core's Salesforce-MCP
/// bearer injection consumes. Authenticates the caller, mints a fresh bearer via
/// the RFC 7523 JWT-bearer grant (acting as the caller's Salesforce username),
/// and returns `{ access_token, instance_url }`.
pub async fn salesforce_token_handler(
    Extension(deps): Extension<SalesforceDeps>,
    headers: HeaderMap,
) -> Response {
    let Ok(session) = extract_user_from_cookie(&headers) else {
        return (StatusCode::UNAUTHORIZED, "authentication required").into_response();
    };
    if !deps.config.is_usable() {
        return (StatusCode::SERVICE_UNAVAILABLE, "Salesforce not configured").into_response();
    }

    // The Salesforce username to act as. For these orgs the SSO email is the
    // Salesforce username; the Connected App must admin-pre-authorize the user.
    match salesforce_jwt_bearer::fetch_token(&deps.config, session.email.as_str()).await {
        Ok(fresh) => Json(TokenResponse {
            access_token: fresh.access_token,
            instance_url: fresh.instance_url,
        })
        .into_response(),
        Err(e) => {
            tracing::error!(error = %e, user_id = %session.user_id, "Salesforce JWT-bearer token mint failed");
            (
                StatusCode::BAD_GATEWAY,
                "Salesforce token acquisition failed",
            )
                .into_response()
        }
    }
}
