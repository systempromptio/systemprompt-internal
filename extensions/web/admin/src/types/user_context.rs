//! The resolved caller identity carried through admin request handling.

use serde::Serialize;
use systemprompt::identifiers::{Email, SessionId, UserId};

#[derive(Debug, Clone, Serialize)]
pub struct UserContext {
    pub user_id: UserId,
    pub username: String,
    pub email: Email,
    pub department: String,
    pub roles: Vec<String>,
    pub is_admin: bool,
    pub email_verified: bool,
    /// The caller's own session id, from the `session_id` JWT claim. `None`
    /// for tokens minted without one.
    pub session_id: Option<SessionId>,
}
