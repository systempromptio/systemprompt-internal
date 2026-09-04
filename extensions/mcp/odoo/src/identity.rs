//! Resolving the acting user's Odoo credential.
//!
//! The MCP transport authenticates the caller as a platform user; this module
//! turns that platform user into an Odoo credential by reading `odoo_identity`
//! — the same table the profile-page link flow writes. There is no fallback
//! account and no shared key: a caller who has not linked Odoo is told to link
//! it, because the alternative is executing their request as somebody else.

use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{ChaCha20Poly1305, Nonce};
use systemprompt::database::DbPool;
use systemprompt::identifiers::UserId;

use crate::client::Credentials;
use crate::error::OdooError;

const NONCE_LEN: usize = 12;

pub const NOT_LINKED_MESSAGE: &str = "You have not linked an Odoo account yet. Open /admin/profile, connect Odoo with your login \
     and an API key from Odoo's Preferences → Account Security, then try again.";

// Why: the same master key the admin plane seals with — the two processes
// share nothing but this value and the framing.
fn master_key() -> Result<[u8; 32], OdooError> {
    // Why: env::var().ok() and SecretsBootstrap::get().ok() are both
    // missing-is-normal carve-outs encoding the priority chain (env var
    // first, then the encrypted bootstrap store).
    let hex_key = std::env::var("ENCRYPTION_MASTER_KEY")
        .ok()
        .or_else(|| {
            systemprompt::config::SecretsBootstrap::get()
                .ok()
                .and_then(|s| s.get("encryption_master_key").cloned())
        })
        .ok_or_else(|| {
            OdooError::NotConfigured(
                "ENCRYPTION_MASTER_KEY is not set; stored Odoo API keys cannot be opened"
                    .to_owned(),
            )
        })?;

    let bytes = hex::decode(hex_key.trim())
        .map_err(|_e| OdooError::Internal("master key is not valid hex".to_owned()))?;
    bytes
        .try_into()
        .map_err(|_e| OdooError::Internal("master key did not decode to 32 bytes".to_owned()))
}

#[doc(hidden)]
pub fn open_api_key(key: &[u8; 32], sealed: &str) -> Result<String, OdooError> {
    let blob = hex::decode(sealed.trim())
        .map_err(|_e| OdooError::Internal("stored Odoo API key is not valid hex".to_owned()))?;
    if blob.len() <= NONCE_LEN {
        return Err(OdooError::Internal(
            "stored Odoo API key is too short to contain a nonce".to_owned(),
        ));
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let cipher = ChaCha20Poly1305::new(key.into());
    let plaintext = cipher
        .decrypt(Nonce::from_slice(nonce_bytes), ciphertext)
        .map_err(|_e| {
            OdooError::Internal("stored Odoo API key could not be decrypted".to_owned())
        })?;
    String::from_utf8(plaintext)
        .map_err(|_e| OdooError::Internal("decrypted Odoo API key is not UTF-8".to_owned()))
}

pub async fn resolve_credentials(
    pool: &DbPool,
    user_id: &UserId,
) -> Result<Credentials, OdooError> {
    let pg_pool = pool
        .pool()
        .ok_or_else(|| OdooError::Internal("no Postgres pool available".to_owned()))?;

    let row = sqlx::query!(
        "SELECT odoo_login, odoo_uid, odoo_api_key_encrypted FROM odoo_identity WHERE user_id = $1",
        user_id.as_str()
    )
    .fetch_optional(pg_pool.as_ref())
    .await
    .map_err(|e| OdooError::Internal(e.to_string()))?;

    let row = row.ok_or_else(|| OdooError::NotLinked(NOT_LINKED_MESSAGE.to_owned()))?;

    Ok(Credentials {
        user_id: user_id.clone(),
        login: row.odoo_login,
        uid: row.odoo_uid,
        api_key: open_api_key(&master_key()?, &row.odoo_api_key_encrypted)?,
    })
}

// Why: Write back a `odoo_uid` that Odoo has just confirmed for this
// credential.
//
// Why this never fails the caller: the tool call it accompanies has already
// succeeded on the refreshed uid. A failure to persist costs one extra
// `authenticate` round trip on the next call and nothing else, so it is
// logged rather than turned into a user-visible error on a request that
// worked.
pub async fn persist_uid(pool: &DbPool, user_id: &UserId, odoo_uid: i32) {
    let Some(pg_pool) = pool.pool() else {
        return;
    };
    let result = sqlx::query!(
        "UPDATE odoo_identity SET odoo_uid = $2, updated_at = CURRENT_TIMESTAMP WHERE user_id = $1",
        user_id.as_str(),
        odoo_uid
    )
    .execute(pg_pool.as_ref())
    .await;
    if let Err(e) = result {
        tracing::warn!(error = %e, %user_id, odoo_uid, "Could not persist the refreshed Odoo uid");
    }
}
