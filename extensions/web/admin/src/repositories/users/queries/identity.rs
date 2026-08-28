//! Who a user actually is, read fresh for screens that must name them.
//!
//! Why this exists next to `find_user_roles_department`: pages reached through
//! the admin auth gate get their `UserContext` from a self-contained JWT, so
//! `email` and `username` are whatever the token carried when it was minted —
//! not what the database says now. That is fine for a greeting and wrong for a
//! consent screen, where the address shown is the whole basis on which an
//! operator decides to hand out a durable credential.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

/// The identity fields a consent screen needs, and nothing else.
#[derive(Debug, Clone)]
pub struct UserIdentity {
    pub email: String,
    pub display_name: String,
    pub is_active: bool,
}

pub async fn find_user_identity(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Option<UserIdentity>, sqlx::Error> {
    let row = sqlx::query!(
        r#"
        SELECT
            u.email,
            COALESCE(u.display_name, u.full_name, u.name) AS "display_name!",
            (u.status = 'active') AS "is_active!"
        FROM users u
        WHERE u.id = $1
        "#,
        user_id.as_str()
    )
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| UserIdentity {
        email: r.email,
        display_name: r.display_name,
        is_active: r.is_active,
    }))
}
