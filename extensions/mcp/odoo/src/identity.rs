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

/// The message shown to a caller with no linked Odoo account. It names the
/// page they need, because "unauthorized" would send an agent looking for a
/// permissions problem that does not exist.
pub const NOT_LINKED_MESSAGE: &str =
    "You have not linked an Odoo account yet. Open /admin/profile, connect Odoo with your login \
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

/// Open a sealed API key: hex of `nonce || ciphertext`, matching the admin
/// plane's `repositories::users::odoo_identity`.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the framing — a mismatch between the two crates would only show up as a
/// runtime decryption failure otherwise. Not part of the public API.
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
        .map_err(|_e| OdooError::Internal("stored Odoo API key could not be decrypted".to_owned()))?;
    String::from_utf8(plaintext)
        .map_err(|_e| OdooError::Internal("decrypted Odoo API key is not UTF-8".to_owned()))
}

/// The acting user's Odoo credential.
///
/// # Errors
/// [`OdooError::NotLinked`] when the user has no `odoo_identity` row — the
/// ordinary case for a first-time caller. Other variants mean the credential
/// exists but could not be opened.
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
        login: row.odoo_login,
        uid: row.odoo_uid,
        api_key: open_api_key(&master_key()?, &row.odoo_api_key_encrypted)?,
    })
}
