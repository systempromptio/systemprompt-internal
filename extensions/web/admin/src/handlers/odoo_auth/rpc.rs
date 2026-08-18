//! The one Odoo JSON-RPC call the admin plane makes: `common.authenticate`.
//!
//! Everything else that talks to Odoo lives in the odoo MCP server crate. This
//! is deliberately not a general client — the link handler needs to prove a
//! credential works and learn its uid before storing it, and nothing more. A
//! shared client crate for a single 40-line call would cost a dependency edge
//! from the admin extension into an MCP binary.

use serde::Deserialize;

/// Where Odoo lives, from the environment or the profile's secrets.
#[derive(Debug, Clone)]
pub struct OdooConnection {
    /// Base URL, no trailing slash, e.g. `https://odoo.example.com`.
    pub url: String,
    /// Database name — an Odoo server can host several.
    pub db: String,
}

pub(crate) const ODOO_URL_ENV: &str = "ODOO_URL";
pub(crate) const ODOO_DB_ENV: &str = "ODOO_DB";

// Why: env::var().ok() and SecretsBootstrap::get().ok() are both
// missing-is-normal carve-outs encoding the priority chain (env var first —
// the container path, where secrets arrive as env — then the profile's
// secrets store, the local path, where `odoo_url` / `odoo_db` live in
// secrets.json as custom keys).
fn setting(env_name: &str, secrets_key: &str) -> Option<String> {
    std::env::var(env_name).ok().or_else(|| {
        systemprompt::config::SecretsBootstrap::get()
            .ok()
            .and_then(|s| s.get(secrets_key).cloned())
    })
}

impl OdooConnection {
    /// Read the connection from `ODOO_URL` / `ODOO_DB`, falling back to the
    /// profile secrets (`odoo_url` / `odoo_db`). `None` when either is unset
    /// or blank — the caller reports "not configured", which is a deployment
    /// problem, not a user error.
    #[must_use]
    pub fn from_env() -> Option<Self> {
        let url = setting(ODOO_URL_ENV, "odoo_url")?;
        let db = setting(ODOO_DB_ENV, "odoo_db")?;
        let url = url.trim().trim_end_matches('/').to_owned();
        let db = db.trim().to_owned();
        if url.is_empty() || db.is_empty() {
            return None;
        }
        Some(Self { url, db })
    }

    /// The JSON-RPC endpoint every call posts to.
    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("{}/jsonrpc", self.url)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum OdooRpcError {
    #[error("Odoo HTTP error: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Odoo returned {status}")]
    Status { status: reqwest::StatusCode },
    #[error("Odoo JSON-RPC fault: {0}")]
    Fault(String),
}

#[derive(Deserialize)]
struct JsonRpcEnvelope {
    // JSON: protocol boundary — the result is `false` on a failed
    // authenticate, an integer uid on success.
    result: Option<serde_json::Value>,
    error: Option<JsonRpcError>,
}

#[derive(Deserialize)]
struct JsonRpcError {
    message: String,
}

// Why: resolves login + api_key to an Odoo uid. Ok(None) means Odoo answered
// normally and said no — a wrong credential, the user's problem. An Err means
// the call did not complete, which is ours or Odoo's.
pub(crate) async fn authenticate(
    conn: &OdooConnection,
    login: &str,
    api_key: &str,
) -> Result<Option<i32>, OdooRpcError> {
    let body = serde_json::json!({
        "jsonrpc": "2.0",
        "method": "call",
        "id": 1,
        "params": {
            "service": "common",
            "method": "authenticate",
            "args": [conn.db, login, api_key, {}],
        }
    });

    // Why: without a request timeout a stalled Odoo hangs the sign-in handler
    // (and the browser) instead of failing with an error the user can act on.
    let resp = reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(5))
        .timeout(std::time::Duration::from_secs(20))
        .build()?
        .post(conn.endpoint())
        .json(&body)
        .send()
        .await?;

    if !resp.status().is_success() {
        return Err(OdooRpcError::Status {
            status: resp.status(),
        });
    }

    let envelope: JsonRpcEnvelope = resp.json().await?;
    if let Some(fault) = envelope.error {
        return Err(OdooRpcError::Fault(fault.message));
    }

    Ok(uid_from_result(envelope.result.as_ref()))
}

/// Read the uid out of an `authenticate` result.
///
/// Odoo answers a rejected credential with JSON `false`, not an error, so the
/// non-integer case is the ordinary "wrong password" path and must not be
/// mistaken for a protocol fault.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the `false` / integer / null cases without a live Odoo; not part of the
/// public API.
#[doc(hidden)]
#[must_use]
pub fn uid_from_result(result: Option<&serde_json::Value>) -> Option<i32> {
    result?.as_i64().and_then(|uid| i32::try_from(uid).ok())
}
