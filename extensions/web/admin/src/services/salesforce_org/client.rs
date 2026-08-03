//! Authenticated Salesforce client: REST, Tooling and Metadata deploy.
//!
//! Auth is the RFC 7523 JWT-bearer grant, reusing
//! [`salesforce_jwt_bearer`](crate::services::salesforce_jwt_bearer). No
//! browser, no refresh token, nothing to rotate but the certificate.
//!
//! The Metadata *deploy* REST resource accepts JWT-format access tokens, which
//! the SOAP Metadata API does not — SOAP rejects them with "SOAP API does not
//! support JWT-based access tokens". That single fact is why this module exists
//! instead of shelling out to the `sf` CLI: deploy over REST keeps the whole
//! loop headless with the credentials the platform already holds.

use crate::handlers::salesforce_auth::SalesforceError;
use crate::services::salesforce_jwt_bearer;

/// Salesforce API version for REST and Tooling *resource paths*.
///
/// Independent of [`METADATA_VERSION`] despite holding the same value today.
/// This one only decides which `/services/data/vNN.0/` URLs are called, so it
/// governs which sObjects exist — `McpServerAccess`, for one, appears at 67.0.
pub const API_VERSION: &str = "67.0";

/// Metadata API *schema* version, emitted as `<version>` in `package.xml`.
///
/// Separate from [`API_VERSION`] because this one selects a schema, not a URL:
/// it decides which elements a deployed component may carry. Bumping it is a
/// deliberate act — the deploy is declarative, so an element that comes newly
/// into scope and is then omitted takes its default rather than being left
/// alone. See `deploy/salesforce/README.md` for the probe method that
/// establishes the accepted element set for a version.
pub const METADATA_VERSION: &str = "67.0";

/// Connection details for the org being read or configured.
///
/// Deliberately separate from `SalesforceConfig`: that describes *this*
/// deployment's SSO client, whereas this describes an arbitrary target org.
#[derive(Clone)]
pub struct TargetOrg {
    /// My Domain base URL, e.g. `https://acme.my.salesforce.com`.
    pub my_domain: String,
    /// External Client App consumer key. Per-org — Salesforce mints it.
    pub consumer_key: String,
    /// Salesforce *Username* to act as. Not the email; the two differ and
    /// Salesforce matches the assertion `sub` on the Username.
    pub jwt_subject: String,
    /// PEM private key matching the certificate uploaded to the app.
    pub private_key_pem: String,
    /// PEM certificate matching [`private_key_pem`](Self::private_key_pem).
    ///
    /// Required to *apply*, unused to export or diff. A metadata deploy is
    /// declarative and `certificate` is in schema on
    /// `ExtlClntAppGlobalOauthSettings`, so a package that omits it clears the
    /// app's digital signature — and with it the JWT-bearer grant this type
    /// authenticates with.
    pub certificate_pem: Option<String>,
}

impl TargetOrg {
    /// Read a target from `SF_TARGET_*` environment variables.
    ///
    /// # Errors
    /// [`SalesforceError::Internal`] naming the first missing variable.
    pub fn from_env() -> Result<Self, SalesforceError> {
        fn var(name: &str) -> Result<String, SalesforceError> {
            std::env::var(name)
                .map_err(|e| SalesforceError::Internal(format!("{name} is unusable: {e}")))
        }
        Ok(Self {
            my_domain: var("SF_TARGET_MY_DOMAIN")?.trim_end_matches('/').to_owned(),
            consumer_key: var("SF_TARGET_CONSUMER_KEY")?,
            jwt_subject: var("SF_TARGET_JWT_SUBJECT")?,
            private_key_pem: var("SF_TARGET_PRIVATE_KEY")?,
            // Why: optional here rather than required, so export and diff still
            // work without it. Apply checks for it and refuses.
            //
            // Falls back to the platform's own certificate — env var, then the
            // profile's secrets store — so configuring the org this deployment
            // already talks to needs no extra plumbing. SF_TARGET_CERTIFICATE
            // stays available for pointing at a *different* org, matching how
            // the other SF_TARGET_* values work.
            certificate_pem: std::env::var("SF_TARGET_CERTIFICATE")
                .ok()
                .or_else(crate::handlers::salesforce_auth::salesforce_certificate),
        })
    }

    fn token_url(&self) -> String {
        format!("{}/services/oauth2/token", self.my_domain)
    }
}

// Why: hand-written rather than derived because the struct holds an RSA
// private key. A derived Debug would print it in full anywhere the value is
// formatted or attached to a tracing span.
impl std::fmt::Debug for TargetOrg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TargetOrg")
            .field("my_domain", &self.my_domain)
            .field("consumer_key", &"<redacted>")
            .field("jwt_subject", &self.jwt_subject)
            .field("private_key_pem", &"<redacted>")
            .field("certificate_pem", &self.certificate_pem.is_some())
            .finish()
    }
}

/// A live, authenticated connection to one org.
pub struct Connection {
    access_token: String,
    instance_url: String,
    http: reqwest::Client,
}

// Why: same reason as TargetOrg — this one holds a live bearer token.
impl std::fmt::Debug for Connection {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection")
            .field("instance_url", &self.instance_url)
            .field("access_token", &"<redacted>")
            .field("http", &"reqwest::Client")
            .finish()
    }
}

impl Connection {
    /// Mint a token and open a connection.
    ///
    /// # Errors
    /// Propagates signing and token-endpoint failures from
    /// [`salesforce_jwt_bearer::fetch_token_with_key`].
    pub async fn connect(target: &TargetOrg) -> Result<Self, SalesforceError> {
        let token = salesforce_jwt_bearer::fetch_token_with_key(
            &target.consumer_key,
            &target.jwt_subject,
            &target.my_domain,
            &target.token_url(),
            &target.private_key_pem,
        )
        .await?;
        Ok(Self {
            access_token: token.access_token,
            instance_url: token.instance_url,
            http: reqwest::Client::new(),
        })
    }

    /// The instance the token is scoped to.
    #[must_use]
    pub fn instance_url(&self) -> &str {
        &self.instance_url
    }

    // Why: A raw authenticated GET returning JSON, exposed to the sibling
    // `deploy` module so it can poll the deploy-status resource. Errors with
    // TokenEndpoint on a non-2xx.
    pub(super) async fn get_json_public(
        &self,
        path: &str,
    ) -> Result<serde_json::Value, SalesforceError> {
        self.get_json(path).await
    }

    // Why: POST a pre-assembled multipart body and return the raw response
    // text. reqwest is built here without the `multipart` feature, so callers
    // assemble the body and this only attaches the boundary header. Errors with
    // TokenEndpoint on a non-2xx.
    pub(super) async fn post_multipart(
        &self,
        path: &str,
        boundary: &str,
        body: Vec<u8>,
    ) -> Result<String, SalesforceError> {
        let resp = self
            .http
            .post(format!("{}{path}", self.instance_url))
            .bearer_auth(&self.access_token)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.is_success() {
            Ok(text)
        } else {
            Err(SalesforceError::TokenEndpoint { status, body: text })
        }
    }

    async fn get_json(&self, path: &str) -> Result<serde_json::Value, SalesforceError> {
        let resp = self
            .http
            .get(format!("{}{path}", self.instance_url))
            .bearer_auth(&self.access_token)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SalesforceError::TokenEndpoint { status, body });
        }
        resp.json().await.map_err(SalesforceError::Http)
    }

    /// Run a SOQL query against the REST API, following `nextRecordsUrl` so the
    /// caller always sees the whole result set rather than the first page.
    ///
    /// # Errors
    /// [`SalesforceError::TokenEndpoint`] on a non-2xx,
    /// [`SalesforceError::Http`] on transport or decode failure.
    pub async fn soql(&self, query: &str) -> Result<Vec<serde_json::Value>, SalesforceError> {
        self.query_paged(query, false).await
    }

    /// As [`soql`](Self::soql), against the Tooling API.
    ///
    /// # Errors
    /// Same as [`soql`](Self::soql).
    pub async fn tooling_soql(
        &self,
        query: &str,
    ) -> Result<Vec<serde_json::Value>, SalesforceError> {
        self.query_paged(query, true).await
    }

    async fn query_paged(
        &self,
        query: &str,
        tooling: bool,
    ) -> Result<Vec<serde_json::Value>, SalesforceError> {
        let prefix = if tooling { "tooling/" } else { "" };
        let mut path = format!(
            "/services/data/v{API_VERSION}/{prefix}query/?q={}",
            urlencoding::encode(query)
        );
        let mut out = Vec::new();
        loop {
            let page = self.get_json(&path).await?;
            if let Some(records) = page.get("records").and_then(|r| r.as_array()) {
                out.extend(records.iter().cloned());
            }
            match page.get("nextRecordsUrl").and_then(|u| u.as_str()) {
                Some(next) => path = next.to_owned(),
                None => return Ok(out),
            }
        }
    }

    /// Create an sObject record, returning its new id.
    ///
    /// # Errors
    /// [`SalesforceError::TokenEndpoint`] carrying Salesforce's error body on a
    /// non-2xx — which is where validation failures surface.
    pub async fn create_sobject(
        &self,
        sobject: &str,
        body: &serde_json::Value,
        tooling: bool,
    ) -> Result<String, SalesforceError> {
        let prefix = if tooling { "tooling/" } else { "" };
        let resp = self
            .http
            .post(format!(
                "{}/services/data/v{API_VERSION}/{prefix}sobjects/{sobject}",
                self.instance_url
            ))
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            return Err(SalesforceError::TokenEndpoint { status, body: text });
        }
        serde_json::from_str::<serde_json::Value>(&text)
            .ok()
            .and_then(|v| v.get("id").and_then(|i| i.as_str()).map(str::to_owned))
            .ok_or_else(|| {
                SalesforceError::Internal(format!("create {sobject} returned no id: {text}"))
            })
    }

    /// Update an sObject record in place.
    ///
    /// Salesforce answers a successful PATCH with 204 and an empty body, so
    /// there is nothing to return.
    ///
    /// # Errors
    /// [`SalesforceError::TokenEndpoint`] carrying Salesforce's error body on a
    /// non-2xx.
    pub async fn update_sobject(
        &self,
        sobject: &str,
        id: &str,
        body: &serde_json::Value,
        tooling: bool,
    ) -> Result<(), SalesforceError> {
        let prefix = if tooling { "tooling/" } else { "" };
        let resp = self
            .http
            .patch(format!(
                "{}/services/data/v{API_VERSION}/{prefix}sobjects/{sobject}/{id}",
                self.instance_url
            ))
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SalesforceError::TokenEndpoint { status, body });
        }
        Ok(())
    }

    /// Delete an sObject record.
    ///
    /// # Errors
    /// [`SalesforceError::TokenEndpoint`] on a non-2xx.
    pub async fn delete_sobject(
        &self,
        sobject: &str,
        id: &str,
        tooling: bool,
    ) -> Result<(), SalesforceError> {
        let prefix = if tooling { "tooling/" } else { "" };
        let resp = self
            .http
            .delete(format!(
                "{}/services/data/v{API_VERSION}/{prefix}sobjects/{sobject}/{id}",
                self.instance_url
            ))
            .bearer_auth(&self.access_token)
            .send()
            .await?;
        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(SalesforceError::TokenEndpoint { status, body });
        }
        Ok(())
    }
}
