//! Persistence for passkey self-registration.
//!
//! Mirrors [`super::federated`]'s provisioning rules — org lookup, seat check,
//! `name = email` — minus the `federated_identities` row: a passkey user has no
//! external identity until they choose to connect one.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;
use systemprompt_web_shared::error::MarketplaceError;

use crate::repositories::organizations;

#[derive(Debug)]
pub struct PasskeyUserRow {
    pub id: UserId,
    pub has_passkey: bool,
}

pub async fn find_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<PasskeyUserRow>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT u.id AS "id: UserId",
               EXISTS(SELECT 1 FROM webauthn_credentials c WHERE c.user_id = u.id) AS "has_passkey!"
        FROM users u
        WHERE LOWER(u.email) = LOWER($1) AND u.status = 'active'
        "#,
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| PasskeyUserRow {
        id: r.id,
        has_passkey: r.has_passkey,
    }))
}

pub async fn count_webauthn_credentials(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<i64, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT COUNT(*) AS "count!" FROM webauthn_credentials WHERE user_id = $1"#,
        user_id.as_str()
    )
    .fetch_one(pool)
    .await?;
    Ok(row.count)
}

// Why: purpose `credential_link` — the browser finishes enrolment through
// core's public `/webauthn/link/{start,finish}` endpoints, which validate
// against this row exactly as they do for CLI-issued setup tokens.
pub async fn insert_setup_token(
    pool: &PgPool,
    user_id: &UserId,
    token_hash: &str,
    expires_at: chrono::DateTime<chrono::Utc>,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO webauthn_setup_tokens (id, user_id, token_hash, purpose, expires_at)
         VALUES ($1, $2, $3, 'credential_link', $4)",
        uuid::Uuid::new_v4().to_string(),
        user_id.as_str(),
        token_hash,
        expires_at,
    )
    .execute(pool)
    .await?;
    Ok(())
}

// Why: `name` is set to the email to sidestep the `users.name` uniqueness
// constraint; `display_name` carries the human-friendly form. `email_verified`
// is set on the SSO-provisioning precedent: the domain allowlist is the gate,
// no mail-based proof of address exists on this deployment.
pub async fn insert_passkey_user(
    pool: &PgPool,
    email: &str,
    display_name: &str,
) -> Result<UserId, MarketplaceError> {
    let org_id = organizations::crud::find_organization_for_email(pool, email).await?;
    if let Some(org_id) = org_id.as_deref() {
        organizations::seats::assert_seat_available(pool, org_id).await?;
    }

    let user_id = uuid::Uuid::new_v4().to_string();
    let roles = vec!["user".to_owned()];
    let mut tx = pool.begin().await?;

    sqlx::query!(
        r#"
        INSERT INTO users (id, name, email, display_name, status, email_verified, roles)
        VALUES ($1, $2, $3, $4, 'active', true, $5)
        "#,
        &user_id,
        email,
        email,
        display_name,
        &roles,
    )
    .execute(&mut *tx)
    .await?;

    if let Some(org_id) = org_id.as_deref() {
        sqlx::query!(
            "INSERT INTO organization_members (user_id, org_id, org_role)
             VALUES ($1, $2, 'member')",
            &user_id,
            org_id,
        )
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(UserId::new(user_id))
}
