//! Pure query- and row-shaping for the `crm.lead` tools: the search domain,
//! the list row, and the detail labels. No I/O — everything here is directly
//! assertable, which is why [`lead_domain`] and [`lead_row`] are exposed to
//! the external test workspace.

use systemprompt::models::artifacts::{Column, ColumnType, TableArtifact};

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

/// One lead, typed at the Odoo wire boundary.
///
/// Field names are Odoo's own — the contract anyone who knows Odoo already
/// speaks. Odoo's JSON quirks are absorbed by the deserializers: `false`
/// means absent, and a many2one arrives as `[id, "Display Name"]` and
/// collapses to its name.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LeadRow {
    pub id: i64,
    #[serde(deserialize_with = "odoo::text", default)]
    pub name: Option<String>,
    #[serde(deserialize_with = "odoo::many2one", default)]
    pub stage_id: Option<String>,
    #[serde(rename = "user_id", deserialize_with = "odoo::many2one", default)]
    pub salesperson: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub partner_name: Option<String>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub email_from: Option<String>,
    #[serde(deserialize_with = "odoo::number", default)]
    pub expected_revenue: Option<f64>,
    #[serde(deserialize_with = "odoo::number", default)]
    pub probability: Option<f64>,
    #[serde(deserialize_with = "odoo::text", default)]
    pub create_date: Option<String>,
}

/// Serde adapters for Odoo's wire idioms, usable by any record struct.
pub mod odoo {
    use serde::{Deserialize, Deserializer};

    pub fn text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        // JSON: protocol boundary — Odoo writes `false` where a field is empty.
        let v = serde_json::Value::deserialize(d)?;
        Ok(match v {
            serde_json::Value::String(s) if !s.trim().is_empty() => Some(s),
            _ => None,
        })
    }

    pub fn many2one<'de, D: Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
        // JSON: protocol boundary — `[id, "Display Name"]`, or `false`.
        let v = serde_json::Value::deserialize(d)?;
        Ok(v.as_array()
            .and_then(|t| t.get(1))
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned))
    }

    pub fn number<'de, D: Deserializer<'de>>(d: D) -> Result<Option<f64>, D::Error> {
        let v = serde_json::Value::deserialize(d)?;
        Ok(v.as_f64())
    }
}

// Why: dashboards consume this as structured rows — a table artifact, not
// prose. The markdown row below stays for the model-facing text rendering;
// machines must never have to regex it back apart.
#[doc(hidden)]
#[must_use]
pub fn lead_table(records: &[serde_json::Value]) -> TableArtifact {
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String).with_header("Subject"),
        Column::new("stage_id", ColumnType::String).with_header("Stage"),
        Column::new("user_id", ColumnType::String).with_header("Salesperson"),
        Column::new("partner_name", ColumnType::String).with_header("Contact"),
        Column::new("email_from", ColumnType::String).with_header("Email"),
        Column::new("expected_revenue", ColumnType::Currency).with_header("Expected revenue"),
        Column::new("probability", ColumnType::Percentage).with_header("Probability"),
        Column::new("create_date", ColumnType::Date).with_header("Created"),
    ];
    // JSON: protocol boundary, both sides — records arrive as the RPC
    // client's JSON and TableArtifact carries rows as JSON values. The typed
    // LeadRow between them is the contract; a record that fails to type is
    // logged and dropped rather than shipped half-parsed.
    let rows = records
        .iter()
        .filter_map(
            |record| match serde_json::from_value::<LeadRow>(record.clone()) {
                Ok(row) => serde_json::to_value(row).ok(),
                Err(e) => {
                    tracing::warn!(error = %e, "crm.lead record did not match LeadRow; dropping");
                    None
                },
            },
        )
        .collect();
    TableArtifact::new(columns).with_rows(rows)
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
