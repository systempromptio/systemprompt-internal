//! Model-level Odoo calls, built on the JSON-RPC transport in [`rpc`].
//!
//! Every method here takes the acting user's [`Credentials`] explicitly rather
//! than holding them on the client. That is not ceremony: the client is shared
//! across all callers of the server, and the credential changes per request. A
//! client that remembered a credential would be one refactor away from
//! executing one user's tool call as another user.
//!
//! The per-model wrappers over [`OdooClient::execute_kw`] live in [`crud`].

pub mod crud;
pub mod rpc;

use serde::Serialize;
use systemprompt::database::DbPool;
use systemprompt::identifiers::UserId;

use crate::apps::{map_access_denied, map_missing_app};
use crate::error::OdooError;
pub use rpc::{ODOO_DB_ENV, ODOO_URL_ENV, OdooConnection};

/// The acting user's Odoo credential, resolved per request from
/// `odoo_identity`.
///
/// `user_id` is carried alongside so a uid discovered to be stale can be
/// written back to the row it came from, addressed by primary key rather than
/// by login.
#[derive(Debug, Clone)]
pub struct Credentials {
    pub user_id: UserId,
    pub login: String,
    pub uid: i32,
    pub api_key: String,
}

#[derive(Debug, Clone)]
pub struct OdooClient {
    http: reqwest::Client,
    conn: OdooConnection,
    // Why: not a credential — the pool is only ever used to write a refreshed
    // `odoo_uid` back to the row it was read from. Optional because the
    // knowledge-bank job path builds a client from the environment alone and
    // has no pool to give; it still self-heals for the duration of the call,
    // it just cannot persist the correction.
    identity_store: Option<DbPool>,
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
    pub fn from_env() -> Result<Self, OdooError> {
        // Why: without a request timeout a stalled Odoo hangs the MCP tool
        // call until the *caller's* client gives up, which surfaces as a
        // silent dashboard timeout instead of a tool error.
        let http = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .map_err(|e| OdooError::Internal(format!("failed to build HTTP client: {e}")))?;
        Ok(Self {
            http,
            conn: OdooConnection::from_env()?,
            identity_store: None,
        })
    }

    // Why: Let this client persist a `odoo_uid` correction it discovers.
    #[must_use]
    pub fn with_identity_store(mut self, pool: DbPool) -> Self {
        self.identity_store = Some(pool);
        self
    }

    #[must_use]
    pub const fn connection(&self) -> &OdooConnection {
        &self.conn
    }

    pub async fn authenticate(&self, login: &str, api_key: &str) -> Result<Option<i32>, OdooError> {
        let args = [
            serde_json::json!(self.conn.db),
            serde_json::json!(login),
            serde_json::json!(api_key),
            serde_json::json!({}),
        ];
        let result = rpc::call(&self.http, &self.conn, "common", "authenticate", &args).await?;
        Ok(result.as_i64().and_then(|uid| i32::try_from(uid).ok()))
    }

    fn execute_kw_args(
        &self,
        uid: i32,
        creds: &Credentials,
        call: &ModelCall<'_>,
    ) -> [serde_json::Value; 7] {
        [
            serde_json::json!(self.conn.db),
            serde_json::json!(uid),
            serde_json::json!(creds.api_key),
            serde_json::json!(call.model),
            serde_json::json!(call.method),
            call.args.clone(),
            call.kwargs.clone(),
        ]
    }

    pub async fn execute_kw(
        &self,
        creds: &Credentials,
        call: ModelCall<'_>,
    ) -> Result<serde_json::Value, OdooError> {
        let first = rpc::call(
            &self.http,
            &self.conn,
            "object",
            "execute_kw",
            &self.execute_kw_args(creds.uid, creds, &call),
        )
        .await;

        // Why: every model call funnels through here, so this is the one place
        // a fault can be recognised while the model name and the acting login
        // are still in hand. Callers get an error naming the app or the
        // credential, not the table.
        let err = match first {
            Ok(value) => return Ok(value),
            Err(e) => map_missing_app(call.model, e),
        };
        let err = map_access_denied(&creds.login, call.model, err);

        // Why: `odoo_uid` is cached in `odoo_identity` and never refreshed, so
        // a uid that stops matching the login — a re-provisioned account, a
        // database restored from elsewhere — makes Odoo raise AccessDenied on
        // a credential that is perfectly good. Left alone that reads as "your
        // key is dead, go relink", which is both wrong and unfixable by the
        // remedy it names. So ask Odoo who this credential is before believing
        // the diagnosis.
        if !matches!(err, OdooError::AccessDenied(_)) {
            return Err(err);
        }
        let Ok(Some(fresh_uid)) = self.authenticate(&creds.login, &creds.api_key).await else {
            return Err(err);
        };
        if fresh_uid == creds.uid {
            return Err(err);
        }
        tracing::warn!(
            login = %creds.login,
            stale_uid = creds.uid,
            fresh_uid,
            "Stored Odoo uid was stale; refreshed it from the credential and retrying"
        );
        if let Some(pool) = &self.identity_store {
            crate::identity::persist_uid(pool, &creds.user_id, fresh_uid).await;
        }
        rpc::call(
            &self.http,
            &self.conn,
            "object",
            "execute_kw",
            &self.execute_kw_args(fresh_uid, creds, &call),
        )
        .await
        .map_err(|e| map_missing_app(call.model, e))
        .map_err(|e| map_access_denied(&creds.login, call.model, e))
    }
}

// JSON: protocol boundary
// Why: execute_kw takes a positional args list whose first element is the id
// list, so the wire shape is `[[id]]`; a flat `[id]` is the classic Odoo
// mistake and unlinks nothing.
#[must_use]
pub fn unlink_args(ids: &[i64]) -> serde_json::Value {
    serde_json::json!([ids])
}
