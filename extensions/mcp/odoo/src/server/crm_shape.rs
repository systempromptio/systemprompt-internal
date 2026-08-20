//! Pure query- and row-shaping for the `crm.lead` tools: the search domain,
//! the list row, and the detail labels. No I/O — everything here is directly
//! assertable, which is why [`lead_domain`] and [`lead_row`] are exposed to
//! the external test workspace.

use crate::format::field_or_dash;
use crate::tools::inputs::LeadSearchInput;

pub(super) const LEAD_LABELS: [(&str, &str); 9] = [
    ("name", "Subject"),
    ("partner_name", "Contact"),
    ("email_from", "Email"),
    ("phone", "Phone"),
    ("stage_id", "Stage"),
    ("user_id", "Salesperson"),
    ("expected_revenue", "Expected revenue"),
    ("probability", "Probability"),
    ("create_date", "Created"),
];

#[doc(hidden)]
#[must_use]
pub fn lead_domain(input: &LeadSearchInput) -> serde_json::Value {
    let mut domain: Vec<serde_json::Value> = Vec::new();

    if let Some(query) = input
        .query
        .as_deref()
        .map(str::trim)
        .filter(|q| !q.is_empty())
    {
        domain.push(serde_json::json!("|"));
        domain.push(serde_json::json!("|"));
        domain.push(serde_json::json!(["name", "ilike", query]));
        domain.push(serde_json::json!(["partner_name", "ilike", query]));
        domain.push(serde_json::json!(["email_from", "ilike", query]));
    }
    if let Some(stage) = input
        .stage
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        domain.push(serde_json::json!(["stage_id.name", "ilike", stage]));
    }
    if let Some(user) = input
        .user
        .as_deref()
        .map(str::trim)
        .filter(|u| !u.is_empty())
    {
        domain.push(serde_json::json!("|"));
        domain.push(serde_json::json!(["user_id.name", "ilike", user]));
        domain.push(serde_json::json!(["user_id.login", "ilike", user]));
    }

    serde_json::Value::Array(domain)
}

#[doc(hidden)]
#[must_use]
pub fn lead_row(record: &serde_json::Value) -> String {
    let id = record
        .get("id")
        .and_then(serde_json::Value::as_i64)
        .unwrap_or_default();
    format!(
        "- **[{id}] {}** — {} · {} · {} · revenue {}",
        field_or_dash(record, "name"),
        field_or_dash(record, "stage_id"),
        field_or_dash(record, "user_id"),
        field_or_dash(record, "partner_name"),
        field_or_dash(record, "expected_revenue"),
    )
}
