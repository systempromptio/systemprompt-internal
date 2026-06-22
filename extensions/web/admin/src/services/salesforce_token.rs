//! Salesforce token banking — read the per-user banked tokens and refresh the
//! access token when stale (or on a caller-reported 401) via Salesforce's
//! refresh-token grant, re-banking the result. Returns a fresh
//! `{access_token, instance_url}` bearer for the Hosted MCP endpoint.
//!
//! Self-contained on the secret-resolution path: the refresh is driven entirely
//! from the banked `instance_url` + `client_id` plus `SALESFORCE_CLIENT_SECRET`,
//! so it needs no SSO config object.

use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::PgPool;

use systemprompt::identifiers::UserId;

use crate::handlers::salesforce_auth::{
    client_secret, post_token_request, SalesforceError, PLUGIN_ID,
};
use crate::repositories::plugins_grp::plugin_env::upsert_plugin_env_var;
use crate::repositories::{secret_crypto, secret_keys};

/// Refresh once the access token is older than this many seconds. Salesforce
/// returns no `expires_in`, so we treat `issued_at` age as the staleness signal.
const REFRESH_TTL_SECS: u64 = 3600;

/// A fresh Salesforce bearer plus the instance it is scoped to.
pub struct FreshToken {
    pub access_token: String,
    pub instance_url: String,
}

/// Return a non-stale Salesforce access token for `user_id`, refreshing it
/// first if it is older than [`REFRESH_TTL_SECS`] or `force` is set (e.g. a
/// caller observed a 401). Newly-minted tokens are re-banked.
pub async fn get_fresh_token(
    pool: &PgPool,
    user_id: &UserId,
    force: bool,
) -> Result<FreshToken, SalesforceError> {
    let banked = read_banked(pool, user_id).await?;

    let access_token = banked
        .get("access_token")
        .filter(|t| !t.is_empty())
        .ok_or(SalesforceError::NoStoredTokens)?;
    let instance_url = banked
        .get("instance_url")
        .filter(|u| !u.is_empty())
        .ok_or(SalesforceError::NoStoredTokens)?;

    if !force && is_fresh(banked.get("issued_at").map(String::as_str)) {
        return Ok(FreshToken {
            access_token: access_token.clone(),
            instance_url: instance_url.clone(),
        });
    }

    refresh(pool, user_id, &banked, instance_url).await
}

/// Drive the refresh-token grant against the banked instance and re-bank the
/// new `access_token`/`issued_at`/`instance_url`.
async fn refresh(
    pool: &PgPool,
    user_id: &UserId,
    banked: &HashMap<String, String>,
    instance_url: &str,
) -> Result<FreshToken, SalesforceError> {
    let refresh_token = banked
        .get("refresh_token")
        .filter(|t| !t.is_empty())
        .ok_or(SalesforceError::NoStoredTokens)?;
    let client_id = banked
        .get("client_id")
        .filter(|c| !c.is_empty())
        .ok_or_else(|| SalesforceError::Internal("banked tokens missing client_id".into()))?;
    let client_secret = client_secret().ok_or(SalesforceError::MissingClientSecret)?;

    let token_url = format!(
        "{}/services/oauth2/token",
        instance_url.trim_end_matches('/')
    );
    let body = format!(
        "grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
        urlencoding::encode(refresh_token),
        urlencoding::encode(client_id),
        urlencoding::encode(&client_secret),
    );
    let refreshed = post_token_request(&token_url, body).await?;

    // The refresh grant keeps the original refresh_token and may return a new
    // instance_url; re-bank only what changed.
    let new_instance = refreshed.instance_url.as_deref().unwrap_or(instance_url);
    upsert_plugin_env_var(
        pool,
        user_id,
        PLUGIN_ID,
        "access_token",
        &refreshed.access_token,
        true,
    )
    .await?;
    if let Some(issued) = refreshed.issued_at.as_deref() {
        upsert_plugin_env_var(pool, user_id, PLUGIN_ID, "issued_at", issued, true).await?;
    }
    upsert_plugin_env_var(pool, user_id, PLUGIN_ID, "instance_url", new_instance, true).await?;

    tracing::info!(user_id = %user_id, "Refreshed Salesforce access token");
    Ok(FreshToken {
        access_token: refreshed.access_token,
        instance_url: new_instance.to_string(),
    })
}

/// `issued_at` is an epoch-millis string; the token is fresh if it is younger
/// than the TTL. A missing/unparseable stamp forces a refresh.
fn is_fresh(issued_at: Option<&str>) -> bool {
    let Some(issued_ms) = issued_at.and_then(|s| s.trim().parse::<u128>().ok()) else {
        return false;
    };
    let Ok(now) = SystemTime::now().duration_since(UNIX_EPOCH) else {
        return false;
    };
    let now_ms = now.as_millis();
    now_ms.saturating_sub(issued_ms) < u128::from(REFRESH_TTL_SECS) * 1000
}

/// Read every banked `salesforce` var for the user, decrypting the encrypted
/// ones under the user's DEK.
async fn read_banked(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<HashMap<String, String>, SalesforceError> {
    let master_key =
        secret_crypto::load_master_key().map_err(|e| SalesforceError::Internal(e.to_string()))?;
    let dek = secret_keys::get_or_create_user_dek(pool, user_id, &master_key)
        .await
        .map_err(|e| SalesforceError::Internal(e.to_string()))?;

    let rows = sqlx::query!(
        "SELECT var_name, var_value, is_secret, encrypted_value, value_nonce \
         FROM plugin_env_vars WHERE user_id = $1 AND plugin_id = $2",
        user_id.as_str(),
        PLUGIN_ID,
    )
    .fetch_all(pool)
    .await
    .map_err(|e| SalesforceError::Internal(e.to_string()))?;

    let mut out = HashMap::new();
    for row in rows {
        let value = if row.is_secret {
            let (Some(enc), Some(nonce_bytes)) = (row.encrypted_value, row.value_nonce) else {
                continue;
            };
            let nonce: [u8; 12] = nonce_bytes
                .as_slice()
                .try_into()
                .map_err(|_| SalesforceError::Internal("invalid nonce length".into()))?;
            let plaintext = secret_crypto::decrypt(&dek, &nonce, &enc)
                .map_err(|e| SalesforceError::Internal(e.to_string()))?;
            String::from_utf8(plaintext).map_err(|e| SalesforceError::Internal(e.to_string()))?
        } else {
            row.var_value
        };
        out.insert(row.var_name, value);
    }
    Ok(out)
}
