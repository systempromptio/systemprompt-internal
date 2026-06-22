//! Federated-identity resolution for external SSO (e.g. Salesforce).
//!
//! Bridges an externally-issued identity (`issuer`, `external_sub`) to a local
//! `users` row, honouring the "merge by verified email" rule that core's own
//! `find_or_create_federated` deliberately omits.
//!
//! Resolution order (the first match wins):
//! 1. **Existing mapping** — the `(issuer, external_sub)` pair already points at
//!    a user (a returning SSO login).
//! 2. **Email link** — an active local account already owns this email. We
//!    attach the federated identity to it instead of minting a duplicate. This
//!    is the account *merge*. The caller MUST have verified `email_verified` and
//!    an allow-listed domain before reaching this path — linking an unverified
//!    address would let a hostile `IdP` claim arbitrary accounts.
//! 3. **Create** — no mapping and no local account: provision a fresh user plus
//!    the mapping in a single transaction.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

/// Outcome of [`resolve_federated_user`]: a local user the caller can mint a
/// session for.
#[derive(Debug, Clone)]
pub struct ResolvedFederatedUser {
    pub user_id: UserId,
    pub email: String,
    pub display_name: String,
    pub roles: Vec<String>,
}

struct LocalUser {
    id: String,
    display_name: String,
    roles: Vec<String>,
}

async fn find_mapping(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
) -> Result<Option<String>, sqlx::Error> {
    let row = sqlx::query!(
        "UPDATE federated_identities SET last_seen_at = CURRENT_TIMESTAMP \
         WHERE issuer = $1 AND external_sub = $2 RETURNING user_id",
        issuer,
        external_sub
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| r.user_id))
}

async fn find_active_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<LocalUser>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id, COALESCE(display_name, name) AS "display_name!", roles AS "roles!: Vec<String>"
        FROM users
        WHERE LOWER(email) = LOWER($1) AND status = 'active'
        "#,
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| LocalUser {
        id: r.id,
        display_name: r.display_name,
        roles: r.roles,
    }))
}

async fn load_user(pool: &PgPool, user_id: &str) -> Result<Option<LocalUser>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id, COALESCE(display_name, name) AS "display_name!", roles AS "roles!: Vec<String>"
        FROM users WHERE id = $1
        "#,
        user_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| LocalUser {
        id: r.id,
        display_name: r.display_name,
        roles: r.roles,
    }))
}

/// Attach `(issuer, external_sub)` to an existing user. Idempotent: a mapping
/// that already exists is left untouched.
async fn link_existing(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    user_id: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) \
         VALUES ($1, $2, $3) ON CONFLICT (issuer, external_sub) DO NOTHING",
        issuer,
        external_sub,
        user_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

/// Provision a brand-new federated user and its mapping in one transaction.
///
/// Only reached for verified, allow-listed emails, so `email_verified` is set
/// `true` and `name` is the (unique) email — sidestepping the `users.name`
/// uniqueness constraint while keeping `display_name` human-friendly.
async fn create_federated(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    email: &str,
    display_name: &str,
) -> Result<ResolvedFederatedUser, sqlx::Error> {
    let user_id = uuid::Uuid::new_v4().to_string();
    let roles = vec!["user".to_string()];
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

    sqlx::query!(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) VALUES ($1, $2, $3)",
        issuer,
        external_sub,
        &user_id,
    )
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(ResolvedFederatedUser {
        user_id: UserId::new(user_id),
        email: email.to_string(),
        display_name: display_name.to_string(),
        roles,
    })
}

/// Resolve an external identity to a local user, linking-or-creating as needed.
///
/// `email` / `display_name` come from the verified `IdP` claims. The caller is
/// responsible for the upstream gate (`email_verified == true` and an
/// allow-listed domain) before invoking this.
pub async fn resolve_federated_user(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    email: &str,
    display_name: &str,
) -> Result<ResolvedFederatedUser, sqlx::Error> {
    // 1. Existing mapping wins outright.
    if let Some(user_id) = find_mapping(pool, issuer, external_sub).await? {
        if let Some(user) = load_user(pool, &user_id).await? {
            return Ok(ResolvedFederatedUser {
                user_id: UserId::new(user.id),
                email: email.to_string(),
                display_name: user.display_name,
                roles: user.roles,
            });
        }
    }

    // 2. Merge into an existing active account that owns this verified email.
    if let Some(user) = find_active_user_by_email(pool, email).await? {
        link_existing(pool, issuer, external_sub, &user.id).await?;
        return Ok(ResolvedFederatedUser {
            user_id: UserId::new(user.id),
            email: email.to_string(),
            display_name: user.display_name,
            roles: user.roles,
        });
    }

    // 3. First touch with no local counterpart — provision.
    create_federated(pool, issuer, external_sub, email, display_name).await
}
