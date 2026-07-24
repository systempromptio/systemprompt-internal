//! Lookup of the identity record behind a signed-in user.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

#[derive(Debug, Clone)]
pub struct UserIdentityRow {
    pub id: String,
    pub name: String,
    pub email: String,
    pub display_name: Option<String>,
    pub roles: Vec<String>,
}

pub async fn find_user_identity(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Option<UserIdentityRow>, sqlx::Error> {
    let row = sqlx::query!(
        r#"SELECT id, name, email, display_name,
                  COALESCE(roles, '{}') as "roles!: Vec<String>"
           FROM users WHERE id = $1"#,
        user_id.as_str(),
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| UserIdentityRow {
        id: r.id,
        name: r.name,
        email: r.email,
        display_name: r.display_name,
        roles: r.roles,
    }))
}
