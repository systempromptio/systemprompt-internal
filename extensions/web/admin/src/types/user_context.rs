//! The resolved caller identity carried through admin request handling.

use serde::Serialize;
use systemprompt::identifiers::{Email, SessionId, UserId};

/// Which credential store this identity came from.
///
/// The dashboard once rendered "signed in as admin" from one store while the
/// data on the page was fetched under another — four were live at once, and
/// they disagreed. There is exactly one now, and the page says which, so a
/// second variant can never be added silently.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum IdentitySource {
    // Why: the admin session cookie, resolved against the same pool the page
    // reads — which is what makes identity and data share one origin.
    SessionCookie,
}

#[derive(Debug, Clone, Serialize)]
pub struct UserContext {
    pub user_id: UserId,
    pub username: String,
    pub email: Email,
    pub department: String,
    pub roles: Vec<String>,
    pub is_admin: bool,
    pub is_platform_admin: bool,
    pub email_verified: bool,
    pub session_id: Option<SessionId>,
    pub source: IdentitySource,
}
