//! Row-level primitives behind [`super::resolve_federated_user`].
//!
//! Kept apart from the resolution policy in the parent module: these are plain
//! single-statement queries, while the parent decides which of them to consult
//! and in what order.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

pub(super) struct LocalUser {
    pub(super) id: UserId,
    pub(super) email: String,
    pub(super) display_name: String,
    pub(super) roles: Vec<String>,
}

pub(super) async fn find_mapping(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
) -> Result<Option<UserId>, sqlx::Error> {
    let row = sqlx::query!(
        "UPDATE federated_identities SET last_seen_at = CURRENT_TIMESTAMP \
         WHERE issuer = $1 AND external_sub = $2 RETURNING user_id",
        issuer,
        external_sub
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| UserId::new(r.user_id)))
}

pub(super) async fn find_active_user_by_email(
    pool: &PgPool,
    email: &str,
) -> Result<Option<LocalUser>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id AS "id: UserId", email, COALESCE(display_name, name) AS "display_name!", roles AS "roles!: Vec<String>"
        FROM users
        WHERE LOWER(email) = LOWER($1) AND status = 'active'
        "#,
        email
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| LocalUser {
        id: r.id,
        email: r.email,
        display_name: r.display_name,
        roles: r.roles,
    }))
}

// Why `odoo_uid` only: it is the identifier Odoo issued and cannot be chosen by
// whoever creates the account. The login can be. Matching on either meant anyone
// able to create an Odoo user with a chosen login could resolve onto the platform
// account holding that login — and this lookup deliberately returns accounts whose
// platform email differs, so the collision was invisible to the caller. A
// re-created Odoo user (new uid, same login) no longer resolves here; that is a
// rare operator event whose remedy is to re-link from the profile page.
//
// Returns `email` so the caller can compare it against the claim. Resolving onto a
// differently-addressed account is a decision the caller must take explicitly,
// not something this query should smuggle past it.
pub(super) async fn find_active_user_by_odoo_uid(
    pool: &PgPool,
    external_sub: &str,
) -> Result<Option<LocalUser>, sqlx::Error> {
    let Ok(odoo_uid) = external_sub.parse::<i32>() else {
        return Ok(None);
    };
    let row = sqlx::query!(
        r#"
        SELECT u.id AS "id: UserId", u.email, COALESCE(u.display_name, u.name) AS "display_name!", u.roles AS "roles!: Vec<String>"
        FROM odoo_identity oi
        JOIN users u ON u.id = oi.user_id
        WHERE oi.odoo_uid = $1
          AND u.status = 'active'
        LIMIT 1
        "#,
        odoo_uid
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| LocalUser {
        id: r.id,
        email: r.email,
        display_name: r.display_name,
        roles: r.roles,
    }))
}

/// One row per external identity a user can currently sign in through.
#[derive(Debug, Clone)]
pub struct FederatedIdentitySummary {
    pub issuer: String,
    pub external_sub: String,
}

/// Lists the external identities bound to a user.
///
/// Read by consent screens so they can state *how* the current session
/// authenticated, not merely which row it landed on. When those two disagree,
/// the operator is the only one who can tell whether that is correct.
pub(super) async fn list_federated_identities(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Vec<FederatedIdentitySummary>, sqlx::Error> {
    let rows = sqlx::query!(
        "SELECT issuer, external_sub FROM federated_identities \
         WHERE user_id = $1 ORDER BY last_seen_at DESC NULLS LAST, issuer",
        user_id.as_str()
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| FederatedIdentitySummary {
            issuer: r.issuer,
            external_sub: r.external_sub,
        })
        .collect())
}

pub(super) async fn load_user(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Option<LocalUser>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id AS "id: UserId", email, COALESCE(display_name, name) AS "display_name!", roles AS "roles!: Vec<String>"
        FROM users WHERE id = $1
        "#,
        user_id.as_str()
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| LocalUser {
        id: r.id,
        email: r.email,
        display_name: r.display_name,
        roles: r.roles,
    }))
}

pub(super) async fn link_existing(
    pool: &PgPool,
    issuer: &str,
    external_sub: &str,
    user_id: &UserId,
) -> Result<(), sqlx::Error> {
    sqlx::query!(
        "INSERT INTO federated_identities (issuer, external_sub, user_id) \
         VALUES ($1, $2, $3) ON CONFLICT (issuer, external_sub) DO NOTHING",
        issuer,
        external_sub,
        user_id.as_str()
    )
    .execute(pool)
    .await?;
    Ok(())
}
