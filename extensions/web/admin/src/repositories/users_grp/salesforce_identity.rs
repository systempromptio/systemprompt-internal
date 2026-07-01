//! Storage for a local user's Salesforce *Username* (userinfo `preferred_username`).
//!
//! The Salesforce JWT-bearer grant matches its `sub` claim against the Salesforce
//! Username, which is not the login email (e.g. `ed.aa…@agentforce.com` vs
//! `ed@systemprompt.io`). The SSO callback captures the Username here so the
//! Hosted-MCP token accessor can mint a bearer as the right user. Lives in the
//! web-owned `salesforce_user_identities` side table (schema/15), not the
//! vendored `federated_identities` table.

use sqlx::PgPool;

/// Record (or refresh) the Salesforce Username for `user_id`. Idempotent: a
/// repeat login overwrites the stored Username and bumps `updated_at`.
pub async fn upsert(pool: &PgPool, user_id: &str, sf_username: &str) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO salesforce_user_identities (user_id, sf_username) \
         VALUES ($1, $2) \
         ON CONFLICT (user_id) DO UPDATE \
         SET sf_username = EXCLUDED.sf_username, updated_at = CURRENT_TIMESTAMP",
        user_id,
        sf_username
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// The Salesforce Username for `user_id`, or `None` if this user never completed
/// a Salesforce SSO login (in which case the caller falls back to the email).
pub async fn find(pool: &PgPool, user_id: &str) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query!(
        "SELECT sf_username FROM salesforce_user_identities WHERE user_id = $1",
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.sf_username))
}
