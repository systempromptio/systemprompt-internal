//! `GET /admin/auth/salesforce/start` — redirect the browser to Salesforce's
//! authorize endpoint with PKCE and an anti-CSRF `state`.

use axum::Extension;
use axum::extract::Query;
use axum::http::HeaderMap;
use axum::http::header::SET_COOKIE;
use axum::response::{IntoResponse, Redirect, Response};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::{
    STATE_COOKIE, SalesforceDeps, login_error, random_url_safe, sanitize_redirect, secure_flag,
};

#[derive(Deserialize)]
pub(crate) struct StartParams {
    redirect: Option<String>,
    mode: Option<String>,
}

pub(crate) async fn salesforce_start(
    Extension(deps): Extension<SalesforceDeps>,
    Query(params): Query<StartParams>,
) -> Response {
    // Why: lint-ok: http-error — an SSO flow reports failure by redirecting back to
    // the login page with ?sso=<reason>; an error status would strand the
    // browser on a dead end instead of returning the user to a usable page.
    let cfg = &deps.config;
    if !cfg.is_usable() {
        return login_error("unavailable");
    }

    let state_token = random_url_safe();
    // Why: PKCE (RFC 7636): a high-entropy verifier kept server-side, and its
    // SHA-256 challenge sent to Salesforce. The token exchange later proves
    // possession of the verifier, so an intercepted code is useless without it.
    let code_verifier = random_url_safe();
    let code_challenge = {
        let digest = Sha256::digest(code_verifier.as_bytes());
        URL_SAFE_NO_PAD.encode(digest)
    };
    let redirect_to = sanitize_redirect(params.redirect);
    // Why: `mode=link` marks the flow as "attach to the signed-in user" — the
    // callback enforces the session; carrying it in the state cookie rather
    // than the OAuth `state` keeps it tamper-evident alongside the nonce.
    let mode_segment = if params.mode.as_deref() == Some("link") {
        "|link"
    } else {
        ""
    };

    // Why: The state cookie carries the CSRF nonce, the PKCE verifier, the
    // post-login target, and the optional flow mode, '|'-separated (base64url
    // values and same-site paths never contain '|').
    let cookie = format!(
        "{STATE_COOKIE}={state_token}|{code_verifier}|{redirect_to}{mode_segment}; Path=/admin/auth/salesforce; HttpOnly; SameSite=Lax; Max-Age=600{}",
        secure_flag()
    );

    let authorize = format!(
        "{}?response_type=code&client_id={}&redirect_uri={}&scope={}&state={}&code_challenge={}&code_challenge_method=S256",
        cfg.authorize_url(),
        urlencoding::encode(&cfg.consumer_key),
        urlencoding::encode(&cfg.redirect_uri),
        urlencoding::encode(&cfg.scopes),
        urlencoding::encode(&state_token),
        urlencoding::encode(&code_challenge),
    );

    let mut headers = HeaderMap::new();
    if let Ok(val) = cookie.parse() {
        headers.insert(SET_COOKIE, val);
    }
    (headers, Redirect::to(&authorize)).into_response()
}
