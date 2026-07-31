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

use std::time::Duration;

use serde::Deserialize;

use crate::handlers::salesforce_auth::SalesforceError;
use crate::services::salesforce_jwt_bearer;

/// Salesforce API version. Bumping this is a deliberate act: the metadata
/// schema is version-sensitive (`isNamedUserJwtEnabled`, for one, is rejected
/// as "not valid in version 64.0").
pub const API_VERSION: &str = "64.0";

const DEPLOY_TIMEOUT: Duration = Duration::from_secs(300);
const DEPLOY_POLL_INTERVAL: Duration = Duration::from_secs(3);

/// Connection details for the org being read or configured.
///
/// Deliberately separate from `SalesforceConfig`: that describes *this*
/// deployment's SSO client, whereas this describes an arbitrary target org.
#[derive(Debug, Clone)]
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
}

impl TargetOrg {
    /// Read a target from `SF_TARGET_*` environment variables.
    ///
    /// # Errors
    /// [`SalesforceError::Internal`] naming the first missing variable.
    pub fn from_env() -> Result<Self, SalesforceError> {
        fn var(name: &str) -> Result<String, SalesforceError> {
            std::env::var(name).map_err(|_| SalesforceError::Internal(format!("{name} is not set")))
        }
        Ok(Self {
            my_domain: var("SF_TARGET_MY_DOMAIN")?.trim_end_matches('/').to_owned(),
            consumer_key: var("SF_TARGET_CONSUMER_KEY")?,
            jwt_subject: var("SF_TARGET_JWT_SUBJECT")?,
            private_key_pem: var("SF_TARGET_PRIVATE_KEY")?,
        })
    }

    fn token_url(&self) -> String {
        format!("{}/services/oauth2/token", self.my_domain)
    }
}

/// A live, authenticated connection to one org.
pub struct Connection {
    access_token: String,
    instance_url: String,
    http: reqwest::Client,
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
