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

pub fn seal_with(key: &[u8; 32], api_key: &str) -> Result<String, OdooIdentityError> {
    let nonce = generate_nonce();
    let ciphertext = encrypt(key, &nonce, api_key.as_bytes())?;
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ciphertext);
    Ok(hex::encode(blob))
}

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

pub async fn list_odoo_logins(pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
    let rows = sqlx::query!("SELECT DISTINCT odoo_login FROM odoo_identity ORDER BY odoo_login")
        .fetch_all(pool)
        .await?;
    Ok(rows.into_iter().map(|r| r.odoo_login).collect())
}

pub async fn delete(pool: &PgPool, user_id: &UserId) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "DELETE FROM odoo_identity WHERE user_id = $1",
        user_id.as_str()
    )
    .execute(pool)
    .await?;
    Ok(())
}
