//! Inbox, history, channel and session-directory reads.
//!
//! The inbox predicate is the isolation boundary. A session sees a message
//! only if it is addressed to that exact session, to the caller personally
//! with no session named, or to a channel the caller belongs to. A message
//! addressed to a *sibling* session of the same user matches none of those
//! arms, which is what stops chatter leaking into unrelated conversations.

use super::{ChannelRow, CommsStore, MessageRow, SessionRow, query};
use crate::error::CommsError;
use systemprompt::identifiers::{SessionId, UserId};

impl CommsStore {
    pub async fn list_inbox(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
        limit: i64,
    ) -> Result<Vec<MessageRow>, CommsError> {
        let pool = self.read()?;
        let rows = sqlx::query_as!(
            MessageRow,
            r#"SELECT m.id,
                     m.sender_user_id AS "sender_user_id: UserId",
                     m.sender_handle,
                     c.slug AS "channel_slug?",
                     m.recipient_user_id AS "recipient_user_id: UserId",
                     m.recipient_session_id AS "recipient_session_id: SessionId",
                     m.delivery_class,
                     m.body,
                     m.thread_id,
                     m.created_at
              FROM comms_messages m
              LEFT JOIN comms_channels c ON c.id = m.channel_id
              LEFT JOIN comms_channel_members cm
                     ON cm.channel_id = m.channel_id AND cm.user_id = $1
              WHERE m.sender_user_id <> $1
                AND (
                      m.recipient_session_id = $2
                   OR (m.recipient_user_id = $1 AND m.recipient_session_id IS NULL)
                   OR (m.channel_id IS NOT NULL AND cm.user_id IS NOT NULL AND NOT cm.muted)
                )
                AND m.created_at > COALESCE(
                      (SELECT r.last_read_at FROM comms_reads r
                        WHERE r.user_id = $1 AND r.session_id = $2 AND r.scope = $3),
                      TIMESTAMPTZ '-infinity')
              ORDER BY m.created_at DESC
              LIMIT $4"#,
            user_id.as_str(),
            session_id.as_str(),
            query::INBOX_SCOPE,
            limit,
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;

        Ok(rows)
    }

    pub async fn mark_inbox_read(
        &self,
        user_id: &UserId,
        session_id: &SessionId,
    ) -> Result<(), CommsError> {
        let pool = self.write()?;
        sqlx::query!(
            r"INSERT INTO comms_reads (user_id, session_id, scope, last_read_at)
              VALUES ($1, $2, $3, NOW())
              ON CONFLICT (user_id, session_id, scope)
              DO UPDATE SET last_read_at = NOW()",
            user_id.as_str(),
            session_id.as_str(),
            query::INBOX_SCOPE,
        )
        .execute(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(())
    }

    pub async fn list_channel_history(
        &self,
        slug: &str,
        limit: i64,
    ) -> Result<Vec<MessageRow>, CommsError> {
        let pool = self.read()?;
        let rows = sqlx::query_as!(
            MessageRow,
            r#"SELECT m.id, m.sender_user_id AS "sender_user_id: UserId", m.sender_handle,
                     c.slug AS "channel_slug?",
                     m.recipient_user_id AS "recipient_user_id: UserId",
                     m.recipient_session_id AS "recipient_session_id: SessionId",
                     m.delivery_class, m.body, m.thread_id, m.created_at
              FROM comms_messages m
              JOIN comms_channels c ON c.id = m.channel_id
              WHERE c.slug = $1
              ORDER BY m.created_at DESC
              LIMIT $2"#,
            slug,
            limit,
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(rows)
    }

    pub async fn list_direct_history(
        &self,
        user_id: &UserId,
        peer_user_id: &UserId,
        limit: i64,
    ) -> Result<Vec<MessageRow>, CommsError> {
        let pool = self.read()?;
        let rows = sqlx::query_as!(
            MessageRow,
            r#"SELECT m.id, m.sender_user_id AS "sender_user_id: UserId", m.sender_handle,
                     NULL::TEXT AS "channel_slug?",
                     m.recipient_user_id AS "recipient_user_id: UserId",
                     m.recipient_session_id AS "recipient_session_id: SessionId",
                     m.delivery_class, m.body, m.thread_id, m.created_at
              FROM comms_messages m
              WHERE m.channel_id IS NULL
                AND ((m.sender_user_id = $1 AND m.recipient_user_id = $2)
                  OR (m.sender_user_id = $2 AND m.recipient_user_id = $1))
              ORDER BY m.created_at DESC
              LIMIT $3"#,
            user_id.as_str(),
            peer_user_id.as_str(),
            limit,
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(rows)
    }

    pub async fn list_channels(&self, user_id: &UserId) -> Result<Vec<ChannelRow>, CommsError> {
        let pool = self.read()?;
        let rows = sqlx::query_as!(
            ChannelRow,
            r"SELECT c.slug, c.name, c.description, c.required_role, c.urgent,
                     COUNT(cm.user_id) AS member_count
              FROM comms_channels c
              LEFT JOIN comms_channel_members cm ON cm.channel_id = c.id
              WHERE c.required_role IS NULL
                 OR EXISTS (SELECT 1 FROM users u
                             WHERE u.id = $1 AND c.required_role = ANY(u.roles))
              GROUP BY c.slug, c.name, c.description, c.required_role, c.urgent
              ORDER BY c.slug",
            user_id.as_str(),
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(rows)
    }

    pub async fn list_live_sessions(&self) -> Result<Vec<SessionRow>, CommsError> {
        let pool = self.read()?;
        let rows = sqlx::query_as!(
            SessionRow,
            r#"SELECT s.handle AS "handle!", s.user_id AS "user_id: UserId", u.display_name, s.workspace,
                      s.git_branch, s.current_activity, s.model, s.last_event_at
               FROM plugin_session_summaries s
               LEFT JOIN users u ON u.id = s.user_id
               WHERE s.handle IS NOT NULL
                 AND s.ended_at IS NULL
                 AND s.last_event_at > NOW() - make_interval(mins => $1)
               ORDER BY s.last_event_at DESC
               LIMIT 200"#,
            super::LIVE_WINDOW_MINUTES,
        )
        .fetch_all(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(rows)
    }
}
