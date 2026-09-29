//! Whether an MCP server is alive, decided from when it last spoke.
//!
//! An MCP server does not report its own health; the traffic through it does.
//! The MCP pages feed this rule the most recent of two facts — the last
//! server-observed tool call and the last touch on a live connection
//! (`mcp_external_sessions` for proxied servers, `mcp_sessions` for in-process
//! ones) — so "alive" means a client actually got through recently.
//!
//! The rule is deliberately generous: alive within two heartbeat intervals,
//! because one missed beat is a slow response and two is a pattern. Anything
//! older is reported as stale rather than dead — a server nobody has used
//! today is not the same claim as a server that has fallen over, and the page
//! must not make the second claim from the first's evidence.

use chrono::{DateTime, Utc};

// Why: how often a busy MCP session is expected to touch its row. Not a
// configured value — nothing in `services/mcp/*.yaml` declares a heartbeat, so
// this is the interval the rule is stated in and the one the unit test pins. A
// server YAML gaining a heartbeat field would replace it.
pub const HEARTBEAT_INTERVAL_SECS: i64 = 300;

/// What the last activity says about one server: spoke within two intervals,
/// spoke longer ago than that, or never spoke at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Liveness {
    Alive,
    Stale,
    Silent,
}

impl Liveness {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Alive => "Alive",
            Self::Stale => "Stale",
            Self::Silent => "Idle",
        }
    }

    // Why: the badge tone the strip paints this state in, kept beside the
    // label so the two cannot describe different states.
    #[must_use]
    pub const fn tone(self) -> &'static str {
        match self {
            Self::Alive => "ok",
            Self::Stale => "warn",
            Self::Silent => "muted",
        }
    }

    #[must_use]
    pub const fn is_alive(self) -> bool {
        matches!(self, Self::Alive)
    }
}

// Why: a beat in the future is alive, not impossible — clock skew between the
// server writing the row and the console reading it is not evidence of a
// problem, and a negative age must not read as silence.
#[must_use]
pub fn liveness_state(
    now: DateTime<Utc>,
    last_heartbeat: Option<DateTime<Utc>>,
    interval_secs: i64,
) -> Liveness {
    let Some(last) = last_heartbeat else {
        return Liveness::Silent;
    };
    if (now - last).num_seconds() <= interval_secs.saturating_mul(2) {
        Liveness::Alive
    } else {
        Liveness::Stale
    }
}
