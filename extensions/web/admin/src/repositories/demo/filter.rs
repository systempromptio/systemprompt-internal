//! Scope shared by every demo query: which user, how far back, how many rows.

use chrono::{DateTime, Duration, Utc};
use systemprompt::identifiers::UserId;

pub const DEFAULT_SINCE_DAYS: i64 = 30;
pub const DEFAULT_LIMIT: i64 = 500;

#[derive(Debug, Clone)]
pub struct DemoFilter {
    pub user_id: Option<UserId>,
    pub since: DateTime<Utc>,
    pub limit: i64,
}

impl DemoFilter {
    pub fn all_users() -> Self {
        Self {
            user_id: None,
            since: Utc::now() - Duration::days(DEFAULT_SINCE_DAYS),
            limit: DEFAULT_LIMIT,
        }
    }

    pub fn for_user(user_id: UserId) -> Self {
        Self {
            user_id: Some(user_id),
            ..Self::all_users()
        }
    }

    #[must_use]
    pub fn since(mut self, since: DateTime<Utc>) -> Self {
        self.since = since;
        self
    }

    #[must_use]
    pub fn limit(mut self, limit: i64) -> Self {
        self.limit = limit;
        self
    }

    pub fn user_filter(&self) -> Option<&str> {
        self.user_id.as_ref().map(UserId::as_str)
    }
}

impl Default for DemoFilter {
    fn default() -> Self {
        Self::all_users()
    }
}
