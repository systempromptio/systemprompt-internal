//! Per-request dependencies shared by the account-linking handlers.
//!
//! There is no external identity provider in this deployment. Accounts are
//! created by an operator, or self-registered against an allow-listed email
//! domain, and the only external system a user links is Odoo — with their own
//! credential, from their profile page, after they already have an account.
//!
//! What is left is the write-capable pool and the self-registration allow-list.

use std::sync::Arc;

use sqlx::PgPool;

/// Comma-separated email domains eligible for passkey self-registration.
///
/// Read from the environment, not a YAML file.
///
/// This is the whole provisioning gate: a wrong value here hands out accounts,
/// so it belongs beside the other deployment secrets rather than in a config
/// file that gets copied between installs.
pub const ALLOWED_DOMAINS_ENV: &str = "SELF_REGISTRATION_EMAIL_DOMAINS";

/// Domains allowed to self-register when [`ALLOWED_DOMAINS_ENV`] is unset.
#[must_use]
pub fn default_allowed_domains() -> Vec<String> {
    vec!["systemprompt.io".to_owned()]
}

/// Parse [`ALLOWED_DOMAINS_ENV`], falling back to [`default_allowed_domains`].
///
/// An empty or whitespace-only value is treated as unset rather than as "allow
/// nobody", so a blank line in an env file cannot silently disable
/// registration.
#[must_use]
pub fn allowed_domains_from_env() -> Vec<String> {
    // Why: env::var().ok() is a missing-is-normal carve-out — an unset
    // variable means "use the default list", not an error.
    let domains: Vec<String> = std::env::var(ALLOWED_DOMAINS_ENV)
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|d| d.trim().to_lowercase())
                .filter(|d| !d.is_empty())
                .collect()
        })
        .unwrap_or_default();

    if domains.is_empty() {
        default_allowed_domains()
    } else {
        domains
    }
}

/// Shared via an axum `Extension` to the profile and registration handlers.
#[derive(Clone)]
pub struct AuthDeps {
    /// Write-capable pool — registration provisions users, linking writes
    /// credentials.
    pub write_pool: Arc<PgPool>,
    /// Email domains eligible for passkey self-registration.
    pub allowed_email_domains: Arc<Vec<String>>,
}

impl std::fmt::Debug for AuthDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthDeps")
            .field("allowed_email_domains", &self.allowed_email_domains)
            .finish_non_exhaustive()
    }
}

impl AuthDeps {
    /// Whether `email` is in an allow-listed domain. `email` is expected
    /// already trimmed and lowercased by the caller.
    #[must_use]
    pub fn email_allowed(&self, email: &str) -> bool {
        email
            .rsplit('@')
            .next()
            .is_some_and(|domain| self.allowed_email_domains.iter().any(|d| d == domain))
    }
}
