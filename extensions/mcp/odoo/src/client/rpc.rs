//! The JSON-RPC transport: one POST to `{ODOO_URL}/jsonrpc`, one envelope in,
//! one envelope out.
//!
//! Odoo's external API is JSON-RPC 2.0 with two services. `common` handles
//! `authenticate`; `object` handles `execute_kw`, which is the door to every
//! model method. Keeping the envelope handling here means the model-level code
//! in the parent module reads as Odoo calls, not as HTTP.

use serde::Deserialize;

use crate::error::OdooError;

/// Where Odoo lives, from the environment.
#[derive(Debug, Clone)]
pub struct OdooConnection {
    /// Base URL, no trailing slash.
    pub url: String,
    /// Database name — one Odoo server can host several.
    pub db: String,
}

pub const ODOO_URL_ENV: &str = "ODOO_URL";
pub const ODOO_DB_ENV: &str = "ODOO_DB";

impl OdooConnection {
    /// Read the connection from `ODOO_URL` / `ODOO_DB`.
    ///
    /// # Errors
    /// [`OdooError::NotConfigured`] when either variable is unset or blank.
    pub fn from_env() -> Result<Self, OdooError> {
        // Why: env::var().ok() twice is a missing-is-normal carve-out — the
        // absent case is reported as NotConfigured, naming both variables.
        let url = std::env::var(ODOO_URL_ENV).ok().unwrap_or_default();
        let db = std::env::var(ODOO_DB_ENV).ok().unwrap_or_default();
        let url = url.trim().trim_end_matches('/').to_owned();
        let db = db.trim().to_owned();
        if url.is_empty() || db.is_empty() {
            return Err(OdooError::NotConfigured(format!(
                "{ODOO_URL_ENV} and {ODOO_DB_ENV} must both be set for the odoo MCP server to \
                 reach Odoo"
            )));
        }
        Ok(Self { url, db })
    }

    #[must_use]
    pub fn endpoint(&self) -> String {
        format!("{}/jsonrpc", self.url)
    }
}

#[derive(Deserialize)]
struct Envelope {
    // JSON: protocol boundary — `result` is whatever the called method
    // returns, so it stays untyped until the caller shapes it.
    result: Option<serde_json::Value>,
    error: Option<Fault>,
}

#[derive(Deserialize)]
struct Fault {
    message: String,
    #[serde(default)]
    data: Option<FaultData>,
}

#[derive(Deserialize)]
struct FaultData {
    #[serde(default)]
    message: Option<String>,
}

/// Build the `params` for a `service.method(args)` call.
///
/// Exposed (behind `#[doc(hidden)]`) so the external test workspace can assert
/// the envelope shape — the positional-args layout is the part of the protocol
/// most easily got wrong — without a live Odoo. Not part of the public API.
#[doc(hidden)]
#[must_use]
pub fn build_request(service: &str, method: &str, args: &[serde_json::Value]) -> serde_json::Value {
    serde_json::json!({
        "jsonrpc": "2.0",
        "method": "call",
        "id": 1,
        "params": { "service": service, "method": method, "args": args },
    })
}

/// Read a JSON-RPC response body into a result value.
///
/// Odoo puts the useful text of a fault in `error.data.message` and a generic
/// class name in `error.message`, so a fault reported with only the latter
/// reads as "Odoo Server Error" and tells the model nothing. Prefer the inner
/// message when it is there.
///
/// Exposed (behind `#[doc(hidden)]`) for the same reason as [`build_request`].
#[doc(hidden)]
pub fn parse_response(body: &str) -> Result<serde_json::Value, OdooError> {
    let envelope: Envelope =
        serde_json::from_str(body).map_err(|e| OdooError::Transport(e.to_string()))?;
    if let Some(fault) = envelope.error {
        let detail = fault
            .data
            .and_then(|d| d.message)
            .map(|m| m.trim().to_owned())
            .filter(|m| !m.is_empty())
            .unwrap_or(fault.message);
        return Err(OdooError::Odoo(detail));
    }
    Ok(envelope.result.unwrap_or(serde_json::Value::Null))
}

/// Post one JSON-RPC call and return its `result`.
pub async fn call(
    http: &reqwest::Client,
    conn: &OdooConnection,
    service: &str,
    method: &str,
    args: &[serde_json::Value],
) -> Result<serde_json::Value, OdooError> {
    let request = build_request(service, method, args);
    let resp = http
        .post(conn.endpoint())
        .json(&request)
        .send()
        .await
        .map_err(|e| OdooError::Transport(e.to_string()))?;

    let status = resp.status();
    let body = resp
        .text()
        .await
        .map_err(|e| OdooError::Transport(e.to_string()))?;

    if !status.is_success() {
        // Why: Odoo answers an application fault with HTTP 200 and an `error`
        // member, so a non-2xx is the proxy, the URL, or the server being
        // down — never a model-level refusal, and the body is not an envelope.
        return Err(OdooError::Transport(format!("Odoo returned HTTP {status}")));
    }

    parse_response(&body)
}
