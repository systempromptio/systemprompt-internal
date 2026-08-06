//! "Link your Odoo account" — the profile-page flow that gives a platform user
//! their own credential in Odoo.
//!
//! There is no OAuth here and no shared service account. Odoo Community's
//! JSON-RPC API authenticates with a login plus an API key the user generates
//! in their own Odoo preferences, so linking is a form post, validated by
//! actually calling `common.authenticate` against the configured database
//! before anything is stored. That is the whole point of the design: the odoo
//! MCP server later executes every tool call with *this* credential, so Odoo's
//! record rules decide what the agent may read and Odoo's audit log names the
//! real person behind it.
//!
//! - [`odoo_link`] validates and stores the credential.
//! - [`odoo_unlink`] removes it.
//! - [`odoo_identity_status`] reports link state for the profile page.
//!
//! Module layout: [`rpc`] (the JSON-RPC call and the connection settings),
//! [`link`], [`unlink`], [`identity`] (one handler each).

mod identity;
mod link;
mod rpc;
mod unlink;

pub(crate) use identity::odoo_identity_status;
pub(crate) use link::odoo_link;
pub(crate) use unlink::odoo_unlink;
pub use rpc::{OdooConnection, OdooRpcError, uid_from_result};

/// Failures from the Odoo account-linking flow.
///
/// Logged once at the HTTP boundary. The client is told which side failed —
/// their credential, our configuration, or Odoo — but never the detail.
#[derive(Debug, thiserror::Error)]
pub enum OdooAuthError {
    #[error("Odoo is not configured: {0}")]
    NotConfigured(String),
    #[error("Odoo rejected the credential")]
    InvalidCredential,
    #[error("Odoo RPC error: {0}")]
    Rpc(#[from] OdooRpcError),
    #[error("Odoo identity storage error: {0}")]
    Storage(#[from] crate::repositories::users::odoo_identity::OdooIdentityError),
}

impl From<OdooAuthError> for crate::error::AdminError {
    fn from(err: OdooAuthError) -> Self {
        match err {
            OdooAuthError::NotConfigured(msg) => Self::Unavailable(msg),
            OdooAuthError::InvalidCredential => Self::Unauthorized(
                "Odoo rejected that login and API key. Check both in Odoo under Preferences → \
                 Account Security."
                    .to_owned(),
            ),
            OdooAuthError::Rpc(e) => Self::Upstream(e.to_string()),
            OdooAuthError::Storage(e) => Self::internal(e),
        }
    }
}
