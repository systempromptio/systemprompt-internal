//! Per-user settings records.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettingsRow {
    pub user_id: UserId,
    pub display_name: Option<String>,
    pub avatar_url: Option<String>,
    pub timezone: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub async fn find_user_settings(
    pool: &PgPool,
    user_id: &UserId,
) -> Result<Option<UserSettingsRow>, sqlx::Error> {
    let id = user_id.as_str();
    sqlx::query_as!(
        UserSettingsRow,
        r#"SELECT
             user_id AS "user_id!: UserId",
             display_name,
             avatar_url,
             timezone,
             created_at,
             updated_at
           FROM user_settings WHERE user_id = $1"#,
        id,
    )
    .fetch_optional(pool)
    .await
}
