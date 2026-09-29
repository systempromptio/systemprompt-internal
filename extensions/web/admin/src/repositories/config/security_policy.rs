//! `services/web/config/security.yaml` — the credential lifetime policy.
//!
//! One knob today: `max_pat_lifetime_days`, the longest a personal access
//! token may live from issuance. The bound is applied by [`bound_pat_expiry`],
//! which is pure so the arithmetic is pinned by a unit test, and read once
//! per process because the file is implementation configuration shipped
//! with the image, not something the console edits.

use std::sync::OnceLock;

use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use systemprompt::config::ProfileBootstrap;
use systemprompt::loader::services_root::ServicesRootBootstrap;

pub const SECURITY_FILE: &str = "web/config/security.yaml";
pub const DEFAULT_MAX_PAT_LIFETIME_DAYS: u32 = 90;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SecurityPolicy {
    #[serde(default = "default_max_pat_lifetime_days")]
    pub max_pat_lifetime_days: u32,
}

const fn default_max_pat_lifetime_days() -> u32 {
    DEFAULT_MAX_PAT_LIFETIME_DAYS
}

impl Default for SecurityPolicy {
    fn default() -> Self {
        Self {
            max_pat_lifetime_days: DEFAULT_MAX_PAT_LIFETIME_DAYS,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct SecurityDoc {
    #[serde(default)]
    security: SecurityPolicy,
}

impl SecurityPolicy {
    // Why: parsed once and cached. A file that fails to parse is logged and
    // served as the default rather than refused: a typo in a lifetime knob
    // must not take the console down, and 90 days is the safe direction.
    #[must_use]
    pub fn get() -> Self {
        static POLICY: OnceLock<SecurityPolicy> = OnceLock::new();
        *POLICY.get_or_init(|| match Self::load() {
            Ok(policy) => policy,
            Err(error) => {
                tracing::error!(error = %error, file = SECURITY_FILE, "security policy unreadable; using defaults");
                Self::default()
            },
        })
    }

    fn load() -> Result<Self, String> {
        let profile = ProfileBootstrap::get().map_err(|e| e.to_string())?;
        let path =
            ServicesRootBootstrap::active_root_or(&profile.paths.services).join(SECURITY_FILE);
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("{}: {e}", path.display())),
        };
        Self::from_yaml(&text)
    }

    pub fn from_yaml(text: &str) -> Result<Self, String> {
        let doc: SecurityDoc = serde_yaml::from_str(text).map_err(|e| e.to_string())?;
        if doc.security.max_pat_lifetime_days == 0 {
            return Err("security.max_pat_lifetime_days must be at least 1".to_owned());
        }
        Ok(doc.security)
    }

    #[must_use]
    pub fn max_pat_lifetime(self) -> Duration {
        Duration::days(i64::from(self.max_pat_lifetime_days))
    }
}

/// Why a requested token expiry was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatExpiryError {
    InPast,
    BeyondPolicy { max_days: u32 },
}

impl std::fmt::Display for PatExpiryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InPast => f.write_str("expires_at must be in the future"),
            Self::BeyondPolicy { max_days } => write!(
                f,
                "expires_at exceeds the maximum token lifetime of {max_days} days"
            ),
        }
    }
}

// Why: the one decision every issuance path makes. No expiry asked for means
// the policy maximum, never "forever"; more than the maximum is refused
// rather than clamped, so the caller learns the policy said no.
pub fn bound_pat_expiry(
    requested: Option<DateTime<Utc>>,
    now: DateTime<Utc>,
    policy: SecurityPolicy,
) -> Result<DateTime<Utc>, PatExpiryError> {
    let ceiling = now + policy.max_pat_lifetime();
    match requested {
        None => Ok(ceiling),
        Some(at) if at <= now => Err(PatExpiryError::InPast),
        Some(at) if at > ceiling => Err(PatExpiryError::BeyondPolicy {
            max_days: policy.max_pat_lifetime_days,
        }),
        Some(at) => Ok(at),
    }
}
