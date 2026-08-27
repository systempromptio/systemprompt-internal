//! The comms persistence layer.
//!
//! Session addressing resolves through the session registry on
//! `plugin_session_summaries`: a handle is only addressable while its session
//! is live, which is what lets a `session`-class send degrade to `inbox`
//! instead of failing when the target has gone away.

pub mod query;
pub mod reads;
pub mod rows;

pub use query::{
    Address, DeliveryClass, INBOX_SCOPE, MAX_BODY_BYTES, channel_scope, check_body, clamp_limit,
    classify, parse_address,
};
pub use rows::{ChannelRow, MessageRow, NewMessage, SentMessage, SessionRow, SessionTarget};

use std::sync::Arc;
use systemprompt::database::DbPool;
use systemprompt::identifiers::{SessionId, UserId};

use crate::error::CommsError;

// Why: minutes of silence after which a session stops being addressable.
pub const LIVE_WINDOW_MINUTES: i32 = 15;

#[derive(Debug, Clone)]
pub struct CommsStore {
    pool: DbPool,
}

impl CommsStore {
    #[must_use]
    pub const fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    fn read(&self) -> Result<Arc<sqlx::PgPool>, CommsError> {
        self.pool
            .pool()
            .ok_or_else(|| CommsError::Internal("no Postgres read pool available".to_owned()))
    }

    fn write(&self) -> Result<Arc<sqlx::PgPool>, CommsError> {
        self.pool
            .write_pool()
            .ok_or_else(|| CommsError::Internal("no Postgres write pool available".to_owned()))
    }

    pub async fn find_user_by_name(&self, name: &str) -> Result<Option<UserId>, CommsError> {
        let pool = self.read()?;
        let row = sqlx::query!(
            r"SELECT id FROM users
              WHERE LOWER(name) = $1 OR LOWER(display_name) = $1 OR LOWER(email) = $1
              ORDER BY id
              LIMIT 1",
            name.to_lowercase(),
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(row.map(|r| UserId::new(r.id)))
    }

    pub async fn find_live_session(
        &self,
        user_id: &UserId,
        handle: &str,
    ) -> Result<Option<SessionTarget>, CommsError> {
        let pool = self.read()?;
        let row = sqlx::query!(
            r"SELECT session_id, user_id
              FROM plugin_session_summaries
              WHERE user_id = $1
                AND handle = $2
                AND ended_at IS NULL
                AND last_event_at > NOW() - make_interval(mins => $3)
              ORDER BY last_event_at DESC
              LIMIT 1",
            user_id.as_str(),
            handle,
            LIVE_WINDOW_MINUTES,
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;

        Ok(row.map(|r| SessionTarget {
            session_id: SessionId::new(r.session_id),
            user_id: UserId::new(r.user_id),
        }))
    }

    pub async fn find_channel_id(&self, slug: &str) -> Result<Option<String>, CommsError> {
        let pool = self.read()?;
        let row = sqlx::query!(r"SELECT id FROM comms_channels WHERE slug = $1", slug)
            .fetch_optional(pool.as_ref())
            .await
            .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(row.map(|r| r.id))
    }

    pub async fn find_session_handle(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<String>, CommsError> {
        let pool = self.read()?;
        let row = sqlx::query!(
            r"SELECT handle FROM plugin_session_summaries WHERE session_id = $1",
            session_id.as_str(),
        )
        .fetch_optional(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;
        Ok(row.and_then(|r| r.handle))
    }

    pub async fn insert_message(
        &self,
        message: &NewMessage<'_>,
    ) -> Result<SentMessage, CommsError> {
        let pool = self.write()?;
        let id = format!("msg_{}", uuid::Uuid::new_v4());
        let row = sqlx::query!(
            r"INSERT INTO comms_messages
                (id, channel_id, sender_user_id, sender_session_id, sender_handle,
                 recipient_user_id, recipient_session_id, delivery_class, body, thread_id)
              VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
              RETURNING id, created_at",
            id,
            message.channel_id,
            message.sender_user_id.as_str(),
            message.sender_session_id.map(SessionId::as_str),
            message.sender_handle,
            message.recipient_user_id.map(UserId::as_str),
            message.recipient_session_id.map(SessionId::as_str),
            message.delivery_class,
            message.body,
            message.thread_id,
        )
        .fetch_one(pool.as_ref())
        .await
        .map_err(|e| CommsError::Internal(e.to_string()))?;

        Ok(SentMessage {
            id: row.id,
            created_at: row.created_at,
        })
    }
}
