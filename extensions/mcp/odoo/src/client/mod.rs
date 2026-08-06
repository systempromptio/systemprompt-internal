//! Model-level Odoo calls, built on the JSON-RPC transport in [`rpc`].
//!
//! Every method here takes the acting user's [`Credentials`] explicitly rather
//! than holding them on the client. That is not ceremony: the client is shared
//! across all callers of the server, and the credential changes per request. A
//! client that remembered a credential would be one refactor away from
//! executing one user's tool call as another user.

pub mod rpc;

use serde::Serialize;

use crate::error::OdooError;
pub use rpc::{OdooConnection, ODOO_DB_ENV, ODOO_URL_ENV};

/// The acting user's Odoo credential, resolved per request from
/// `odoo_identity`.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub login: String,
    pub uid: i32,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub struct OdooClient {
    http: reqwest::Client,
    conn: OdooConnection,
}

/// Optional keyword arguments for a `search_read`.
#[derive(Debug, Default, Serialize)]
pub struct SearchOptions {
    pub fields: Vec<String>,
    pub limit: u32,
    pub order: Option<String>,
}

/// One `execute_kw` invocation: which model method, with which positional and
/// keyword arguments.
///
/// Bundled rather than passed as four parameters because `execute_kw` is the
/// call every other method here funnels through, and a positional list that
/// long is easy to transpose — `model` and `method` are both strings.
#[derive(Debug)]
pub struct ModelCall<'a> {
    pub model: &'a str,
    pub method: &'a str,
    pub args: serde_json::Value,
    pub kwargs: serde_json::Value,
}

/// A `read_group` aggregation: what to count, and how to bucket it.
#[derive(Debug)]
pub struct GroupQuery<'a> {
    pub domain: serde_json::Value,
    pub fields: &'a [&'a str],
    pub group_by: &'a [&'a str],
}

impl OdooClient {
    /// # Errors
    /// [`OdooError::NotConfigured`] when `ODOO_URL` / `ODOO_DB` are not set.
    pub fn from_env() -> Result<Self, OdooError> {
        Ok(Self {
            http: reqwest::Client::new(),
            conn: OdooConnection::from_env()?,
        })
    }

    #[must_use]
    pub const fn connection(&self) -> &OdooConnection {
        &self.conn
    }

    /// Resolve a login + API key to an Odoo uid. `Ok(None)` is a rejected
    /// credential — Odoo answers that with `false`, not a fault.
    ///
    /// # Errors
    /// Transport or protocol failures.
    pub async fn authenticate(
        &self,
        login: &str,
        api_key: &str,
    ) -> Result<Option<i32>, OdooError> {
        let args = [
            serde_json::json!(self.conn.db),
            serde_json::json!(login),
            serde_json::json!(api_key),
            serde_json::json!({}),
        ];
        let result = rpc::call(&self.http, &self.conn, "common", "authenticate", &args).await?;
        Ok(result.as_i64().and_then(|uid| i32::try_from(uid).ok()))
    }

    /// The general door: `object.execute_kw(db, uid, key, model, method, args,
    /// kwargs)`.
    ///
    /// # Errors
    /// Transport failures, or an Odoo fault — which includes access-rule
    /// refusals, and those are the interesting ones: they mean the acting
    /// user genuinely may not do this.
    pub async fn execute_kw(
        &self,
        creds: &Credentials,
        call: ModelCall<'_>,
    ) -> Result<serde_json::Value, OdooError> {
        let rpc_args = [
            serde_json::json!(self.conn.db),
            serde_json::json!(creds.uid),
            serde_json::json!(creds.api_key),
            serde_json::json!(call.model),
            serde_json::json!(call.method),
            call.args,
            call.kwargs,
        ];
        rpc::call(&self.http, &self.conn, "object", "execute_kw", &rpc_args).await
    }

    /// `search_read` — the list form. Returns the raw record array.
    ///
    /// # Errors
    /// As [`execute_kw`](Self::execute_kw).
    pub async fn search_read(
        &self,
        creds: &Credentials,
        model: &str,
        domain: serde_json::Value,
        options: &SearchOptions,
    ) -> Result<Vec<serde_json::Value>, OdooError> {
        let mut kwargs = serde_json::Map::new();
        kwargs.insert("fields".to_owned(), serde_json::json!(options.fields));
        kwargs.insert("limit".to_owned(), serde_json::json!(options.limit));
        if let Some(order) = &options.order {
            kwargs.insert("order".to_owned(), serde_json::json!(order));
        }
        let result = self
            .execute_kw(creds, ModelCall {
                model,
                method: "search_read",
                args: serde_json::json!([domain]),
                kwargs: serde_json::Value::Object(kwargs),
            })
            .await?;
        Ok(result.as_array().cloned().unwrap_or_default())
    }

    /// `read` — fetch named fields for known ids.
    ///
    /// # Errors
    /// As [`execute_kw`](Self::execute_kw).
    pub async fn read(
        &self,
        creds: &Credentials,
        model: &str,
        ids: &[i64],
        fields: &[&str],
    ) -> Result<Vec<serde_json::Value>, OdooError> {
        let result = self
            .execute_kw(creds, ModelCall {
                model,
                method: "read",
                args: serde_json::json!([ids]),
                kwargs: serde_json::json!({ "fields": fields }),
            })
            .await?;
        Ok(result.as_array().cloned().unwrap_or_default())
    }

    /// `read_group` — the aggregation form.
    ///
    /// # Errors
    /// As [`execute_kw`](Self::execute_kw).
    pub async fn read_group(
        &self,
        creds: &Credentials,
        model: &str,
        query: GroupQuery<'_>,
    ) -> Result<Vec<serde_json::Value>, OdooError> {
        let result = self
            .execute_kw(creds, ModelCall {
                model,
                method: "read_group",
                args: serde_json::json!([query.domain, query.fields, query.group_by]),
                kwargs: serde_json::json!({ "lazy": false }),
            })
            .await?;
        Ok(result.as_array().cloned().unwrap_or_default())
    }

    /// `create` — returns the new record id.
    ///
    /// # Errors
    /// As [`execute_kw`](Self::execute_kw); also [`OdooError::Odoo`] if Odoo
    /// answers with something that is not an id.
    pub async fn create(
        &self,
        creds: &Credentials,
        model: &str,
        values: serde_json::Value,
    ) -> Result<i64, OdooError> {
        let result = self
            .execute_kw(creds, ModelCall {
                model,
                method: "create",
                args: serde_json::json!([values]),
                kwargs: serde_json::json!({}),
            })
            .await?;
        result.as_i64().ok_or_else(|| {
            OdooError::Odoo(format!("create on {model} returned no record id"))
        })
    }

    /// `write` — returns Odoo's boolean acknowledgement.
    ///
    /// # Errors
    /// As [`execute_kw`](Self::execute_kw).
    pub async fn write(
        &self,
        creds: &Credentials,
        model: &str,
        id: i64,
        values: serde_json::Value,
    ) -> Result<bool, OdooError> {
        let result = self
            .execute_kw(creds, ModelCall {
                model,
                method: "write",
                args: serde_json::json!([[id], values]),
                kwargs: serde_json::json!({}),
            })
            .await?;
        Ok(result.as_bool().unwrap_or(false))
    }

    /// `message_post` — log a note on any record that inherits `mail.thread`.
    /// Posted as the acting user, so Odoo attributes it to them.
    ///
    /// # Errors
    /// As [`execute_kw`](Self::execute_kw). A model without a message thread
    /// surfaces as an Odoo fault, which is the honest answer.
    pub async fn message_post(
        &self,
        creds: &Credentials,
        model: &str,
        res_id: i64,
        body: &str,
    ) -> Result<i64, OdooError> {
        let result = self
            .execute_kw(creds, ModelCall {
                model,
                method: "message_post",
                args: serde_json::json!([[res_id]]),
                kwargs: serde_json::json!({ "body": body, "message_type": "comment" }),
            })
            .await?;
        Ok(result.as_i64().unwrap_or_default())
    }
}
