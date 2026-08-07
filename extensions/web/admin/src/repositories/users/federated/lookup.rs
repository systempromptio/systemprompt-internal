//! Row-level primitives behind [`super::resolve_federated_user`].
//!
//! Kept apart from the resolution policy in the parent module: these are plain
//! single-statement queries, while the parent decides which of them to consult
//! and in what order.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

pub(super) struct LocalUser {
    pub(super) id: UserId,
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
        SELECT id AS "id: UserId", COALESCE(display_name, name) AS "display_name!", roles AS "roles!: Vec<String>"
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

// Why: matches on `odoo_uid` (the stable identifier Odoo issued as
// `external_sub`) or, as a fallback, the Odoo login — a re-created Odoo user
// keeps its login but not its uid.
pub(super) async fn find_active_user_by_odoo_identity(
    pool: &PgPool,
    external_sub: &str,
    odoo_login: &str,
) -> Result<Option<LocalUser>, sqlx::Error> {
    let odoo_uid: Option<i32> = external_sub.parse().ok();
    let row = sqlx::query!(
        r#"
        SELECT u.id AS "id: UserId", COALESCE(u.display_name, u.name) AS "display_name!", u.roles AS "roles!: Vec<String>"
        FROM odoo_identity oi
        JOIN users u ON u.id = oi.user_id
        WHERE (oi.odoo_uid = $1 OR LOWER(oi.odoo_login) = LOWER($2))
          AND u.status = 'active'
        ORDER BY (oi.odoo_uid = $1) DESC
        LIMIT 1
        "#,
        odoo_uid,
        odoo_login
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| LocalUser {
        id: r.id,
        display_name: r.display_name,
        roles: r.roles,
    }))
}

pub(super) async fn load_user(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Option<LocalUser>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT id AS "id: UserId", COALESCE(display_name, name) AS "display_name!", roles AS "roles!: Vec<String>"
        FROM users WHERE id = $1
        "#,
        user_id.as_str()
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| LocalUser {
        id: r.id,
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
