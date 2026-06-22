//! Salesforce SSO connection config, loaded from
//! `services/web/config/salesforce.yaml`.

use serde::Deserialize;

/// Resolve the Salesforce Connected App secret, env var first then the encrypted
/// secrets store, mirroring [`crate::repositories::secret_crypto::load_master_key`].
/// The secret is never persisted in `salesforce.yaml`.
pub fn client_secret() -> Option<String> {
    // Why: env::var().ok() and SecretsBootstrap::get().ok() are both
    // missing-is-normal carve-outs encoding the priority chain (env var
    // first, then bootstrap).
    std::env::var("SALESFORCE_CLIENT_SECRET").ok().or_else(|| {
        systemprompt::config::SecretsBootstrap::get()
            .ok()
            .and_then(|s| s.get("salesforce_client_secret").cloned())
    })
}

/// Default scopes — `openid`/`email`/`profile` drive login; `api` covers direct
/// REST calls; `refresh_token` + `mcp_api` are what let the banked token reach
/// the Salesforce *Hosted* MCP endpoint (an OAuth resource server that demands a
/// Salesforce bearer with `mcp_api` on every call).
pub(super) fn default_scopes() -> String {
    "openid email profile api refresh_token mcp_api".to_string()
}

/// Mirrors the registration gate in [`crate::handlers::public_register`].
pub(super) fn default_allowed_domains() -> Vec<String> {
    vec![
        "astounddigital.com".to_string(),
        "astoundcommerce.com".to_string(),
    ]
}

/// Salesforce SSO connection config.
///
/// The client *secret* is never stored here — it is read from the
/// `SALESFORCE_CLIENT_SECRET` environment variable at callback/refresh time.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SalesforceConfig {
    #[serde(default)]
    pub enabled: bool,
    /// The org's My Domain base URL, e.g. `https://astound.my.salesforce.com`.
    /// Doubles as the federated-identity `issuer` key.
    pub my_domain: String,
    pub client_id: String,
    /// Must exactly match the Connected App callback, e.g.
    /// `https://example.com/admin/auth/salesforce/callback`.
    pub redirect_uri: String,
    #[serde(default = "default_scopes")]
    pub scopes: String,
    #[serde(default = "default_allowed_domains")]
    pub allowed_email_domains: Vec<String>,
}

impl SalesforceConfig {
    /// A disabled placeholder used when no `salesforce.yaml` is present, so the
    /// routes can still be registered and report "unavailable" cleanly.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            my_domain: String::new(),
            client_id: String::new(),
            redirect_uri: String::new(),
            scopes: default_scopes(),
            allowed_email_domains: default_allowed_domains(),
        }
    }

    pub(super) const fn is_usable(&self) -> bool {
        self.enabled
            && !self.my_domain.is_empty()
            && !self.client_id.is_empty()
            && !self.redirect_uri.is_empty()
    }

    fn base(&self) -> &str {
        self.my_domain.trim_end_matches('/')
    }

    pub(super) fn authorize_url(&self) -> String {
        format!("{}/services/oauth2/authorize", self.base())
    }

    pub(super) fn token_url(&self) -> String {
        format!("{}/services/oauth2/token", self.base())
    }

    pub(super) fn userinfo_url(&self) -> String {
        format!("{}/services/oauth2/userinfo", self.base())
    }

    /// The `issuer` value recorded in `federated_identities`.
    pub(super) fn issuer(&self) -> &str {
        self.base()
    }

    pub(super) fn email_allowed(&self, email: &str) -> bool {
        email
            .rsplit('@')
            .next()
            .is_some_and(|domain| self.allowed_email_domains.iter().any(|d| d == domain))
    }
}
