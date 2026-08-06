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

/// The Odoo app that owns `model`, when it is one we can name.
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

/// Translate a missing-model fault on `model` into a message naming its app.
///
/// Any other error passes through untouched — this must not swallow a genuine
/// access-rule refusal, which mentions the model too but means something
/// entirely different.
#[must_use]
pub fn map_missing_app(model: &str, err: OdooError) -> OdooError {
    let OdooError::Odoo(message) = &err else {
        return err;
    };
    if !is_missing_model_fault(model, message) {
        return err;
    }
    let detail = app_for_model(model).map_or_else(
        || format!("this Odoo instance has no model '{model}'; the app providing it is not installed"),
        |app| {
            format!(
                "Odoo app '{app}' is not installed on this instance, so '{model}' does not \
                 exist. Ask an Odoo administrator to install it from Apps."
            )
        },
    );
    OdooError::AppMissing(detail)
}
