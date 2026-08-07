//! Fixed-window attempt limiter for the Odoo sign-in endpoint.
//!
//! Passkey sign-in had no brute-force surface: possession of the authenticator
//! is the credential, and there is nothing to guess. Odoo sign-in accepts a
//! password, so it does, and the endpoint is unauthenticated by definition.
//!
//! In-process and non-persistent, which is the honest scope: it blunts online
//! guessing against one instance, and is not a substitute for Odoo's own
//! lockout policy. Keys are counted independently, so a shared NAT egress
//! cannot lock out an account and one account cannot exhaust an address's
//! budget for everyone behind it.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Attempts permitted per key within [`WINDOW`].
const MAX_ATTEMPTS: u32 = 10;

/// Width of the counting window.
const WINDOW: Duration = Duration::from_secs(15 * 60);

/// Entries are swept once the map grows past this, so a spray across many
/// distinct logins cannot grow it without bound between sweeps.
const SWEEP_THRESHOLD: usize = 1024;

#[derive(Debug, Clone, Copy)]
struct Window {
    started: Instant,
    attempts: u32,
}

/// Counts failed sign-in attempts per key.
#[derive(Debug, Default)]
pub struct LoginThrottle {
    windows: Mutex<HashMap<String, Window>>,
}

impl LoginThrottle {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether `key` has already spent its budget for the current window.
    ///
    /// Checked before the Odoo round trip, so a throttled caller costs us no
    /// upstream request.
    #[must_use]
    pub fn is_blocked(&self, key: &str) -> bool {
        let now = Instant::now();
        let guard = self.lock();
        guard
            .get(key)
            .is_some_and(|w| now.duration_since(w.started) < WINDOW && w.attempts >= MAX_ATTEMPTS)
    }

    /// Count one failed attempt against `key`.
    pub fn record_failure(&self, key: &str) {
        let now = Instant::now();
        let mut guard = self.lock();

        if guard.len() > SWEEP_THRESHOLD {
            guard.retain(|_, w| now.duration_since(w.started) < WINDOW);
        }

        guard
            .entry(key.to_owned())
            .and_modify(|w| {
                if now.duration_since(w.started) >= WINDOW {
                    *w = Window {
                        started: now,
                        attempts: 1,
                    };
                } else {
                    w.attempts = w.attempts.saturating_add(1);
                }
            })
            .or_insert(Window {
                started: now,
                attempts: 1,
            });
    }

    /// Clear `key`'s budget after a successful sign-in, so a user who
    /// mistyped several times is not left throttled once they get in.
    pub fn record_success(&self, key: &str) {
        self.lock().remove(key);
    }

    // Why: a panic in one request must not poison sign-in for the process. The
    // counter is advisory, so the recovered map is safe to keep using.
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Window>> {
        self.windows
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}
