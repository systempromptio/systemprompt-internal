//! Row shapes returned by the comms store.

use serde::{Deserialize, Serialize};
use systemprompt::identifiers::{SessionId, UserId};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageRow {
    pub id: String,
    pub sender_user_id: UserId,
    pub sender_handle: Option<String>,
    pub channel_slug: Option<String>,
    pub recipient_user_id: Option<UserId>,
    pub recipient_session_id: Option<SessionId>,
    pub delivery_class: String,
    pub body: String,
    pub thread_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChannelRow {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub required_role: Option<String>,
    pub urgent: bool,
    pub member_count: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionRow {
    pub handle: String,
    pub user_id: UserId,
    pub display_name: Option<String>,
    pub workspace: Option<String>,
    pub git_branch: Option<String>,
    pub current_activity: Option<String>,
    pub model: Option<String>,
    pub last_event_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone)]
pub struct SessionTarget {
    pub session_id: SessionId,
    pub user_id: UserId,
}

#[derive(Debug, Clone)]
pub struct NewMessage<'a> {
    pub sender_user_id: &'a UserId,
    pub sender_session_id: Option<&'a SessionId>,
    pub sender_handle: Option<&'a str>,
    pub channel_id: Option<&'a str>,
    pub recipient_user_id: Option<&'a UserId>,
    pub recipient_session_id: Option<&'a SessionId>,
    pub delivery_class: &'a str,
    pub body: &'a str,
    pub thread_id: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub struct SentMessage {
    pub id: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
