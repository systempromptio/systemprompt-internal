//! Storage for a local user's Odoo credentials.
//!
//! Odoo is the CRM system of record, and the odoo MCP server executes every
//! JSON-RPC call as the *calling* user rather than a shared service account —
//! so Odoo's own record rules and audit trail govern what an agent can see and
//! change. That requires holding a per-user credential, which is what this
//! table (`odoo_identity`, schema/15) is for.
//!
//! The API key never leaves this module in plaintext: [`insert`] seals it with
//! ChaCha20-Poly1305 under the deployment master key, and nothing in the admin
//! plane ever opens it again — the odoo MCP server is the only reader, in its
//! own process. A deployment with no master key configured cannot link an Odoo
//! account at all, which is better than banking the key in the clear.

use chrono::{DateTime, Utc};
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::repositories::secrets::secret_crypto::{
    SecretCryptoError, decrypt, encrypt, generate_nonce, load_master_key,
};

/// The link state shown on the profile page: who this user is in Odoo, never
/// the credential itself.
#[derive(Debug, Clone)]
pub struct OdooIdentity {
    pub odoo_login: String,
    pub odoo_uid: i32,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum OdooIdentityError {
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("Odoo API key could not be sealed: {0}")]
    Crypto(#[from] SecretCryptoError),
    #[error("Stored Odoo API key is malformed")]
    MalformedCiphertext,
}

const NONCE_LEN: usize = 12;

/// Seal an API key under `key`: hex of `nonce || ciphertext`.
///
/// A per-row random nonce means the same API key linked by two users never
/// produces the same stored bytes.
///
/// The key is a parameter rather than loaded inside, so the framing can be
/// round-tripped in a test without a configured deployment master key — and so
/// the odoo MCP crate, which opens these values in a different process, has a
/// single documented format to agree with.
///
/// # Errors
/// [`OdooIdentityError::Crypto`] if the AEAD refuses.
pub fn seal_with(key: &[u8; 32], api_key: &str) -> Result<String, OdooIdentityError> {
    let nonce = generate_nonce();
    let ciphertext = encrypt(key, &nonce, api_key.as_bytes())?;
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ciphertext);
    Ok(hex::encode(blob))
}

/// Open a value produced by [`seal_with`].
///
/// # Errors
/// [`OdooIdentityError::MalformedCiphertext`] for bad framing,
/// [`OdooIdentityError::Crypto`] for a failed authentication tag.
pub fn open_with(key: &[u8; 32], sealed: &str) -> Result<String, OdooIdentityError> {
    let blob = hex::decode(sealed.trim()).map_err(|e| {
        tracing::warn!(error = %e, "Stored Odoo API key is not valid hex");
        OdooIdentityError::MalformedCiphertext
    })?;
    if blob.len() <= NONCE_LEN {
        return Err(OdooIdentityError::MalformedCiphertext);
    }
    let (nonce_bytes, ciphertext) = blob.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce_bytes
        .try_into()
        .map_err(|_e| OdooIdentityError::MalformedCiphertext)?;
    let plaintext = decrypt(key, &nonce, ciphertext)?;
    String::from_utf8(plaintext).map_err(|e| {
        tracing::warn!(error = %e, "Decrypted Odoo API key is not valid UTF-8");
        OdooIdentityError::MalformedCiphertext
    })
}

fn seal(api_key: &str) -> Result<String, OdooIdentityError> {
    seal_with(&load_master_key()?, api_key)
}

/// Link (or re-link) `user_id` to an Odoo account. Idempotent: re-linking
/// overwrites the login, uid and key, which is also how a rotated API key is
/// applied.
pub async fn insert(
    pool: &PgPool,
    user_id: &UserId,
    odoo_login: &str,
    odoo_uid: i32,
    api_key: &str,
) -> Result<(), OdooIdentityError> {
    let sealed = seal(api_key)?;
    sqlx::query!(
        "INSERT INTO odoo_identity (user_id, odoo_login, odoo_uid, odoo_api_key_encrypted) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id) DO UPDATE \
         SET odoo_login = EXCLUDED.odoo_login, \
         odoo_uid = EXCLUDED.odoo_uid, \
         odoo_api_key_encrypted = EXCLUDED.odoo_api_key_encrypted, \
         updated_at = CURRENT_TIMESTAMP",
        user_id.as_str(),
        odoo_login,
        odoo_uid,
        sealed
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Link `user_id` to an Odoo account only when no link exists yet. Used by the
/// sign-in auto-link, where the credential may be a password: bootstrapping a
/// first link is helpful, but overwriting an API key the user deliberately
/// stored from the profile page with a sign-in password would break every
/// later RPC call if Odoo enforces API keys (2FA).
pub async fn insert_if_absent(
    pool: &PgPool,
    user_id: &UserId,
    odoo_login: &str,
    odoo_uid: i32,
    api_key: &str,
) -> Result<(), OdooIdentityError> {
    let sealed = seal(api_key)?;
    sqlx::query!(
        "INSERT INTO odoo_identity (user_id, odoo_login, odoo_uid, odoo_api_key_encrypted) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id) DO NOTHING",
        user_id.as_str(),
        odoo_login,
        odoo_uid,
        sealed
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// The link state for `user_id`, or `None` if this user has never linked Odoo.
pub async fn find(pool: &PgPool, user_id: &UserId) -> Result<Option<OdooIdentity>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT odoo_login, odoo_uid, updated_at FROM odoo_identity WHERE user_id = $1",
        user_id.as_str()
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| OdooIdentity {
        odoo_login: r.odoo_login,
        odoo_uid: r.odoo_uid,
        updated_at: r.updated_at,
    }))
}

/// Every Odoo login this deployment knows about, sorted. The answer to "whose
/// work will show up in Odoo's audit log" without touching any credential.
pub async fn list_odoo_logins(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query!("SELECT DISTINCT odoo_login FROM odoo_identity ORDER BY odoo_login")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.odoo_login).collect())
}

/// Unlink `user_id` from Odoo (the profile "Disconnect" flow). An absent row is
/// fine — the state is already what the caller asked for.
pub async fn delete(pool: &PgPool, user_id: &UserId) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM odoo_identity WHERE user_id = $1",
        user_id.as_str()
    )
    .execute(pool)
    .await?;
    Ok(())
}
