//! Identity reads behind `comms_whoami`: the account row, the Odoo link
//! (login and uid only — never the key), and the caller's own live sessions.

use super::{CommsStore, SessionRow};
use crate::error::CommsError;
use systemprompt::identifiers::UserId;

#[derive(Debug, Clone)]
pub struct IdentityRow {
    pub id: UserId,
    pub email: String,
    pub display_name: Option<String>,
    pub roles: Vec<String>,
    pub department: Option<String>,
}

#[derive(Debug, Clone)]
pub struct OdooLinkRow {
    pub odoo_login: String,
    pub odoo_uid: i32,
    pub linked_at: chrono::DateTime<chrono::Utc>,
}

impl CommsStore {
    pub async fn find_identity(&self, user_id: &UserId) -> Result<Option<IdentityRow>, CommsError> {
        let pool = self.read()?;
        let row = sqlx::query!(
            r#"SELECT u.id, u.email, COALESCE(u.display_name, u.name) AS "display_name?",
                      u.roles AS "roles!: Vec<String>", upe.department
               FROM users u
               LEFT JOIN user_profile_ext upe ON upe.user_id = u.id
               WHERE u.id = $1"#,
            user_id.as_str(),
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(row.map(|r| IdentityRow {
            id: UserId::new(r.id),
            email: r.email,
            display_name: r.display_name,
            roles: r.roles,
            department: r.department,
        }))
    }

    pub async fn find_odoo_link(
        &self,
        user_id: &UserId,
    ) -> Result<Option<OdooLinkRow>, CommsError> {
        let pool = self.read()?;
        let row = sqlx::query!(
            r"SELECT odoo_login, odoo_uid, created_at FROM odoo_identity WHERE user_id = $1",
            user_id.as_str(),
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(row.map(|r| OdooLinkRow {
            odoo_login: r.odoo_login,
            odoo_uid: r.odoo_uid,
            linked_at: r.created_at,
        }))
    }

    pub async fn list_own_live_sessions(
        &self,
        user_id: &UserId,
    ) -> Result<Vec<SessionRow>, CommsError> {
        let pool = self.read()?;
        let rows = sqlx::query_as!(
            SessionRow,
            r#"SELECT s.handle AS "handle!", s.user_id AS "user_id: UserId", u.display_name, s.workspace,
                      s.git_branch, s.current_activity, s.model, s.last_event_at
               FROM plugin_session_summaries s
               LEFT JOIN users u ON u.id = s.user_id
               WHERE s.user_id = $1
                 AND s.handle IS NOT NULL
                 AND s.ended_at IS NULL
                 AND s.last_event_at > NOW() - make_interval(mins => $2)
               ORDER BY s.last_event_at DESC
               LIMIT 50"#,
            user_id.as_str(),
            super::LIVE_WINDOW_MINUTES,
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(rows)
    }
}
