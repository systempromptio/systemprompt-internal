//! Rendering Odoo records as the markdown the model reads.
//!
//! Odoo's JSON has two habits that make raw output hard to read. Empty values
//! come back as `false` rather than null, and a many2one relation comes back as
//! `[id, "Display Name"]`. Passing either through unfiltered wastes context and
//! invites a model to report "false" as a customer's phone number.

use systemprompt::models::artifacts::{CliArtifact, TextArtifact};

#[must_use]
pub fn field(record: &serde_json::Value, key: &str) -> Option<String> {
    match record.get(key)? {
        serde_json::Value::Bool(_) | serde_json::Value::Null => None,
        serde_json::Value::String(s) if s.trim().is_empty() => None,
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Number(n) => Some(n.to_string()),
        // Why: the many2one shape. The display name is the second element;
        // the id alone is rarely what a reader wants to see.
        serde_json::Value::Array(items) => {
            items.get(1).and_then(|v| v.as_str()).map(ToOwned::to_owned)
        },
        other @ serde_json::Value::Object(_) => Some(other.to_string()),
    }
}

#[must_use]
pub fn relation_id(record: &serde_json::Value, key: &str) -> Option<i64> {
    record.get(key)?.as_array()?.first()?.as_i64()
}

#[must_use]
pub fn field_or_dash(record: &serde_json::Value, key: &str) -> String {
    field(record, key).unwrap_or_else(|| "—".to_owned())
}

#[must_use]
pub fn detail_lines(record: &serde_json::Value, keys: &[(&str, &str)]) -> String {
    keys.iter()
        .filter_map(|(key, label)| {
            field(record, key).map(|value| format!("- **{label}:** {value}"))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Why: the markdown builders above join sections with blank lines, which
// leaves a leading or trailing one whenever a section comes back empty. The
// renderer treats every line as content, so an unnoticed blank line at either
// end became visible dead space at the top or bottom of the card.
#[must_use]
pub fn text_artifact(title: &str, body: &str) -> CliArtifact {
    CliArtifact::text(TextArtifact::new(body.trim()).with_title(title))
}

#[must_use]
pub fn empty_result(what: &str) -> String {
    format!("No {what} matched. This is Odoo's answer for your account, not an error.")
}
