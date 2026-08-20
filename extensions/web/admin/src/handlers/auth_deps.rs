//! Per-request dependencies shared by the account-linking handlers.
//!
//! Odoo is the identity provider for this deployment: signing in proves an Odoo
//! credential and provisions the platform account on first use. Operators keep
//! a separate passkey route, and its self-registration allow-list still governs
//! who may enrol that way — the Odoo door is gated by Odoo's own user list, not
//! by the allow-list.
//!
//! Linking an Odoo credential *for agents to act with* remains a separate,
//! explicit step on the profile page; signing in does not store the secret.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt::oauth::OAuthRepository;

use super::odoo_auth::LoginThrottle;

pub const ALLOWED_DOMAINS_ENV: &str = "SELF_REGISTRATION_EMAIL_DOMAINS";

#[must_use]
pub fn default_allowed_domains() -> Vec<String> {
    vec!["systemprompt.io".to_owned()]
}

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

/// Shared via an axum `Extension` to the profile, registration and sign-in
/// handlers.
#[derive(Clone)]
pub struct AuthDeps {
    pub write_pool: Arc<PgPool>,
    pub allowed_email_domains: Arc<Vec<String>>,
    pub oauth_repo: Arc<OAuthRepository>,
    pub login_throttle: Arc<LoginThrottle>,
}

impl std::fmt::Debug for AuthDeps {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthDeps")
            .field("allowed_email_domains", &self.allowed_email_domains)
            .finish_non_exhaustive()
    }
}

impl AuthDeps {
    #[must_use]
    pub fn email_allowed(&self, email: &str) -> bool {
        email
            .rsplit('@')
            .next()
            .is_some_and(|domain| self.allowed_email_domains.iter().any(|d| d == domain))
    }
}
