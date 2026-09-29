//! Reads and revocations for the device fleet: bridge sessions, personal
//! access tokens, device certificates, and enrolment links still unclaimed.
//!
//! The four are one page because they are one question — "what is holding a
//! credential to this platform, and is it still alive?" — asked of four
//! tables. They are separate modules because nothing joins them: a bridge
//! session is a heartbeat, a token is a secret, a certificate is a key, and a
//! link is a promise. Only the person they belong to is common, so every row
//! type carries a `UserId` and the page renders it as the same link.
//!
//! Presence is one rule, [`bridge_presence`], applied to a heartbeat. It
//! lives here rather than in the handler so the count on the KPI tile, the
//! badge on the row and the user page can never disagree about what online
//! or stale means.

pub mod certs;
pub mod links;
pub mod pats;
pub mod sessions;
pub mod stats;

use chrono::{DateTime, Utc};

use crate::repositories::overview::liveness::liveness_state;

// Why: a bridge that has not called home in a week is not a bridge anyone is
// using; it is a credential still valid on a machine nobody is watching. Seven
// days is a working week, so a laptop closed on Friday is not reported as
// abandoned on Monday.
pub const STALE_AFTER_DAYS: i64 = 7;

// Why: the desktop bridge beats every 30 s while it runs (its
// `HEARTBEAT_INTERVAL`), so "online now" is a beat inside two of those; the
// window is the liveness rule's, stated here in the bridge's interval.
pub const BRIDGE_HEARTBEAT_SECS: i64 = 30;

/// What a bridge's last heartbeat says: beating now, quiet for less than a
/// week, or silent longer than that.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgePresence {
    Online,
    Idle,
    Stale,
}

impl BridgePresence {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Online => "Online",
            Self::Idle => "Idle",
            Self::Stale => "Stale",
        }
    }

    #[must_use]
    pub const fn tone(self) -> &'static str {
        match self {
            Self::Online => "ok",
            Self::Idle => "muted",
            Self::Stale => "warn",
        }
    }

    #[must_use]
    pub const fn is_online(self) -> bool {
        matches!(self, Self::Online)
    }
}

#[must_use]
pub fn bridge_presence(now: DateTime<Utc>, last_heartbeat: DateTime<Utc>) -> BridgePresence {
    if liveness_state(now, Some(last_heartbeat), BRIDGE_HEARTBEAT_SECS).is_alive() {
        BridgePresence::Online
    } else if (now - last_heartbeat).num_days() < STALE_AFTER_DAYS {
        BridgePresence::Idle
    } else {
        BridgePresence::Stale
    }
}
