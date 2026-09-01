//! Recognising a model that belongs to an Odoo app nobody installed.
//!
//! Odoo Community is modular: Calendar, Project and Discuss are separate apps,
//! and an instance that has not installed one has no table for its models. The
//! JSON-RPC fault that comes back says `Object calendar.event doesn't exist`,
//! which reads like a bug in the caller — a model receiving it will retry, or
//! report that the record was not found, when the truth is that this deployment
//! cannot answer the question at all until an administrator installs the app.
//!
//! So faults of that shape are translated once, here, into
//! [`OdooError::AppMissing`] naming the app rather than the table.
//!
//! The same treatment is owed to `Access Denied`, and it is worth being exact
//! about what that fault means, because the name invites the wrong reading.
//! In Odoo `AccessDenied` is an *authentication* failure — the credential did
//! not identify anyone. A *permission* failure is `AccessError`, whose text
//! names the document. Read as "you lack rights" it sends the user to an
//! administrator to be granted access they already hold, while the credential
//! that actually failed goes unexamined. That is not hypothetical: it is how
//! a stale stored credential was read as a missing CRM group. So the two are
//! mapped separately.

use crate::error::OdooError;

// Why: model prefix to the app name an Odoo administrator would search for in
// Apps. Prefixes rather than exact models, because one missing app takes out
// every model it owns and the message is the same for all of them.
const APPS: [(&str, &str); 6] = [
    ("calendar.", "Calendar"),
    ("project.", "Project"),
    ("discuss.", "Discuss"),
    ("mail.channel", "Discuss"),
    ("crm.", "CRM"),
    ("account.", "Invoicing"),
];

#[must_use]
pub fn app_for_model(model: &str) -> Option<&'static str> {
    APPS.iter()
        .find(|(prefix, _)| model.starts_with(prefix))
        .map(|(_, app)| *app)
}

// Why: the two shapes Odoo uses for "no such model". The first is the ORM's own
// message; the second is what a bare KeyError on the model registry looks like
// once it reaches the JSON-RPC boundary.
fn is_missing_model_fault(model: &str, message: &str) -> bool {
    message.contains(&format!("Object {model} doesn't exist"))
        || (message.contains("KeyError") && message.contains(model))
        || message.contains(&format!("Model not found: {model}"))
}

// Why: `AccessDenied` is raised by res.users._check_credentials — the key did
// not authenticate. It says nothing about groups.
fn is_auth_failure_fault(message: &str) -> bool {
    message.contains("AccessDenied") || message.contains("Access Denied")
}

// Why: the genuine rights refusal, which names the document rather than the
// credential.
fn is_permission_fault(message: &str) -> bool {
    message.contains("AccessError")
        || message.contains("not allowed to access this document")
        || message.contains("not allowed to modify this document")
}

// Why: recognise both refusals while the acting login and model are still in
// hand. Left raw either reaches the user as an opaque internal error naming
// neither the account nor the remedy — and the two remedies are different
// enough that guessing wrong wastes the user's afternoon.
#[must_use]
pub fn map_access_denied(login: &str, model: &str, err: OdooError) -> OdooError {
    let OdooError::Odoo(message) = &err else {
        return err;
    };
    if is_auth_failure_fault(message) {
        return OdooError::AccessDenied(format!(
            "Odoo did not accept the stored credential for '{login}', so the call to '{model}' \
             was never authorised. This is an authentication failure, not a permissions one — \
             the account's Odoo rights are irrelevant until it authenticates, so granting it \
             more access will not help. The stored credential is wrong or has gone stale (an \
             Odoo password change or a re-provisioned account both do this). Open \
             /admin/profile and relink. An API key from Odoo's Preferences → Account Security \
             is the durable choice, because it survives a password change."
        ));
    }
    if is_permission_fault(message) {
        let app = app_for_model(model).unwrap_or("the relevant");
        return OdooError::AccessDenied(format!(
            "Odoo authenticated '{login}' but refused it access to '{model}'. Ask an Odoo \
             administrator to grant that account access to the {app} app (Settings → Users, then \
             the app's access rights)."
        ));
    }
    err
}

#[must_use]
pub fn map_missing_app(model: &str, err: OdooError) -> OdooError {
    let OdooError::Odoo(message) = &err else {
        return err;
    };
    if !is_missing_model_fault(model, message) {
        return err;
    }
    let detail = app_for_model(model).map_or_else(
        || {
            format!(
                "this Odoo instance has no model '{model}'; the app providing it is not installed"
            )
        },
        |app| {
            format!(
                "Odoo app '{app}' is not installed on this instance, so '{model}' does not \
                 exist. Ask an Odoo administrator to install it from Apps."
            )
        },
    );
    OdooError::AppMissing(detail)
}
