//! "Sign in with Salesforce" — OAuth 2.0 / OIDC authorization-code login, plus
//! per-user Salesforce *token banking* for the Hosted MCP server.
//!
//! - [`salesforce_start`] / [`salesforce_callback`] drive the browser login.
//! - The callback banks the user's Salesforce `access_token`/`refresh_token`/
//!   `instance_url`/`issued_at` in the per-user encrypted secret store so agents
//!   can later reach the Salesforce Hosted MCP endpoint as that user.
//! - [`salesforce_token_handler`] is the typed accessor core's external-MCP
//!   client calls to obtain a fresh `{access_token, instance_url}` bearer.
//!
//! Module layout: [`config`] (the loaded YAML), [`start`] (the authorize
//! redirect), [`callback`] (token exchange → identity → session → token bank),
//! [`tokens`] (token shapes, exchange, persistence, the accessor endpoint).

mod callback;
mod config;
mod start;
mod tokens;

use std::sync::Arc;

use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::Rng;
use sqlx::PgPool;

use systemprompt::models::Config;
use systemprompt::oauth::SessionCreationService;

pub use config::{client_secret, SalesforceConfig};
pub use start::salesforce_start;
pub use tokens::salesforce_token_handler;

// Re-exported for callers that need the callback entrypoint directly.
pub use callback::salesforce_callback;

/// Plugin id under which Salesforce tokens are banked in `plugin_env_vars`.
pub const PLUGIN_ID: &str = "salesforce";

// Token plumbing reused by the refresh service in `crate::services`.
pub use tokens::post_token_request;

pub(super) const STATE_COOKIE: &str = "sf_oauth_state";
const DEFAULT_REDIRECT: &str = "/admin";

/// Errors from the Salesforce OAuth/token plumbing. Logged once at the HTTP
/// boundary; the browser only ever sees an opaque `?sso=<reason>`.
#[derive(Debug, thiserror::Error)]
pub enum SalesforceError {
    #[error("Salesforce HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Salesforce token endpoint returned {status}: {body}")]
    TokenEndpoint {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("Salesforce userinfo endpoint returned {0}")]
    UserInfo(reqwest::StatusCode),
    #[error("No Salesforce tokens banked for this user — sign in with Salesforce first")]
    NoStoredTokens,
    #[error("SALESFORCE_CLIENT_SECRET is not set")]
    MissingClientSecret,
    #[error("Salesforce token store error: {0}")]
    Storage(#[from] systemprompt_web_shared::error::MarketplaceError),
    #[error("Salesforce token plumbing: {0}")]
    Internal(String),
}

/// Per-request dependencies handed to the Salesforce handlers via an axum
/// `Extension`. All fields are `Arc`, so cloning is cheap.
#[derive(Clone)]
pub struct SalesforceDeps {
    pub config: Arc<SalesforceConfig>,
    /// Write-capable pool — the callback may provision a user and bank tokens.
    pub write_pool: Arc<PgPool>,
    pub session_service: Arc<SessionCreationService>,
}

impl std::fmt::Debug for SalesforceDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SalesforceDeps")
            .field("config", &self.config)
            .finish_non_exhaustive()
    }
}

pub(super) fn secure_flag() -> &'static str {
    if Config::get().map_or(true, |c| c.use_https) {
        "; Secure"
    } else {
        ""
    }
}

/// Reject anything that isn't a same-site absolute path, to avoid open-redirect.
pub(super) fn sanitize_redirect(raw: Option<String>) -> String {
    match raw {
        Some(r) if r.starts_with('/') && !r.starts_with("//") => r,
        _ => DEFAULT_REDIRECT.to_string(),
    }
}

pub(super) fn login_error(reason: &str) -> Response {
    Redirect::to(&format!("/admin/login?sso={reason}")).into_response()
}

/// 32 random bytes as base64url-no-pad (43 chars) — a valid PKCE verifier and a
/// fine CSRF nonce.
pub(super) fn random_url_safe() -> String {
    let bytes: [u8; 32] = rand::rng().random();
    URL_SAFE_NO_PAD.encode(bytes)
}

/// Parse the state cookie into `(state, code_verifier, redirect_to)`.
pub(super) fn read_state_cookie(headers: &HeaderMap) -> Option<(String, String, String)> {
    let raw = headers
        .get_all("cookie")
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|s| s.split(';'))
        .map(str::trim)
        .find_map(|kv| kv.strip_prefix(&format!("{STATE_COOKIE}=")))?;
    let mut parts = raw.splitn(3, '|');
    let state = parts.next()?.to_string();
    let verifier = parts.next()?.to_string();
    let redirect = parts.next()?.to_string();
    Some((state, verifier, sanitize_redirect(Some(redirect))))
}
