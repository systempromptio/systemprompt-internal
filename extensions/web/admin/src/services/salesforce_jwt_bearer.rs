//! Outbound Salesforce token acquisition via the RFC 7523 JWT-bearer grant.
//!
//! Replaces per-user token *banking* with an on-demand exchange: build and sign
//! a short-lived JWT assertion with the Connected App's private key, POST it to
//! Salesforce's `/services/oauth2/token` under
//! `grant_type=urn:ietf:params:oauth:grant-type:jwt-bearer`, and return the
//! fresh `{access_token, instance_url}` bearer the Salesforce Hosted-MCP host
//! needs. No tokens are stored: every call mints a fresh one.
//!
//! Operational prerequisite: the Connected App must have the matching digital
//! certificate uploaded with "Use digital signatures" enabled, and the user
//! must be admin-pre-authorized. The private key is provisioned as
//! `SALESFORCE_PRIVATE_KEY` (PEM) — see [`salesforce_private_key`].
//!
//! [`salesforce_private_key`]: crate::handlers::salesforce_auth::salesforce_private_key

use std::time::{SystemTime, UNIX_EPOCH};

use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;

use crate::handlers::salesforce_auth::{
    post_token_request, salesforce_private_key, SalesforceConfig, SalesforceError,
};

pub struct FreshToken {
    pub access_token: String,
    pub instance_url: String,
}

/// Lifetime of the signed assertion. Salesforce requires `exp` within 5 minutes
/// of issuance; a short window bounds replay of the assertion itself.
const ASSERTION_TTL_SECS: u64 = 180;

/// RFC 7523 assertion claims for the Salesforce JWT-bearer flow. `iss` is the
/// Connected App consumer key, `sub` the Salesforce username to act as, `aud`
/// the org login host.
#[derive(Debug, Serialize)]
struct Assertion {
    iss: String,
    sub: String,
    aud: String,
    exp: u64,
}

/// Mint a fresh Salesforce access token for `username` via the JWT-bearer grant.
///
/// `username` is the Salesforce Username to act as — the userinfo
/// `preferred_username` captured at SSO login (e.g. `ed.aa…@agentforce.com`), NOT
/// the login email; the two differ and Salesforce matches `sub` on the Username.
/// The External Client App must have the user admin-pre-authorized.
///
/// # Errors
/// - [`SalesforceError::MissingPrivateKey`] if `SALESFORCE_PRIVATE_KEY` is unset.
/// - [`SalesforceError::Internal`] if the key is not valid PEM or signing fails.
/// - [`SalesforceError::TokenEndpoint`] / [`SalesforceError::Http`] on a failed POST.
pub async fn fetch_token(
    cfg: &SalesforceConfig,
    username: &str,
) -> Result<FreshToken, SalesforceError> {
    let private_key_pem = salesforce_private_key().ok_or(SalesforceError::MissingPrivateKey)?;

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| SalesforceError::Internal(format!("system clock before epoch: {e}")))?
        .as_secs();

    let assertion = Assertion {
        iss: cfg.client_id.clone(),
        sub: username.to_owned(),
        aud: cfg.jwt_bearer_audience().to_owned(),
        exp: now + ASSERTION_TTL_SECS,
    };

    let key = EncodingKey::from_rsa_pem(private_key_pem.as_bytes())
        .map_err(|e| SalesforceError::Internal(format!("invalid SALESFORCE_PRIVATE_KEY: {e}")))?;
    let signed = encode(&Header::new(Algorithm::RS256), &assertion, &key)
        .map_err(|e| SalesforceError::Internal(format!("assertion signing failed: {e}")))?;

    let body = format!(
        "grant_type={}&assertion={}",
        urlencoding::encode("urn:ietf:params:oauth:grant-type:jwt-bearer"),
        urlencoding::encode(&signed),
    );
    let resp = post_token_request(&cfg.token_url(), body).await?;

    // The JWT-bearer grant returns the instance the token is scoped to; fall
    // back to the org base if Salesforce omits it.
    let instance_url = resp
        .instance_url
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| cfg.jwt_bearer_audience().to_owned());

    Ok(FreshToken {
        access_token: resp.access_token,
        instance_url,
    })
}
