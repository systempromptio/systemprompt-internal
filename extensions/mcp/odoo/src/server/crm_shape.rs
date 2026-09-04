//! Pure query- and row-shaping for the `crm.lead` tools.
//!
//! The search domain and order, the list row, the tag-name join, and the
//! detail labels. No I/O — everything here is directly assertable, which is why
//! [`lead_domain`], [`lead_order`], [`attach_tag_names`] and [`lead_row`] are
//! exposed to the external test workspace.

use std::collections::HashMap;

use systemprompt::models::artifacts::{Column, ColumnType, TableArtifact, TableHints};

use crate::format::field_or_dash;
use crate::tools::inputs::{LeadSearchInput, LeadSort};

pub(super) const LEAD_LABELS: [(&str, &str); 10] = [
    ("name", "Subject"),
    ("partner_name", "Contact"),
    ("email_from", "Email"),
    ("phone", "Phone"),
    ("stage_id", "Stage"),
    ("user_id", "Salesperson"),
    ("expected_revenue", "Expected revenue"),
    ("probability", "Probability"),
    ("create_date", "Created"),
    ("date_deadline", "Expected close"),
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
    // Why: "open" in Odoo's CRM is two flags, not a stage list — a lost lead
    // is archived (`active = false`) and a won one keeps `active` but sits in
    // a stage flagged `is_won`. Filtering by stage name would break the day
    // someone renames "Won".
    if input.open_only == Some(true) {
        domain.push(serde_json::json!(["active", "=", true]));
        domain.push(serde_json::json!(["stage_id.is_won", "=", false]));
    }
    if let Some(tag) = input
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|t| !t.is_empty())
    {
        domain.push(serde_json::json!(["tag_ids.name", "ilike", tag]));
    }

    serde_json::Value::Array(domain)
}

#[doc(hidden)]
#[must_use]
pub fn lead_order(input: &LeadSearchInput) -> String {
    match input.sort {
        Some(LeadSort::Deadline) => "date_deadline asc, create_date desc".to_owned(),
        Some(LeadSort::Created) | None => "create_date desc".to_owned(),
    }
}

/// One lead, typed at the Odoo wire boundary.
///
/// Field names are Odoo's own — the contract anyone who knows Odoo already
/// speaks. Odoo's JSON quirks are absorbed by the deserializers: `false`
/// means absent, and a many2one arrives as `[id, "Display Name"]` and
/// collapses to its name. `tag_ids` is the one relation Odoo ships as bare
/// ids; `tags` is empty off the wire and filled by [`attach_tag_names`] from
/// a single `crm.tag` read, so dashboards never see an id without its name.
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
    #[serde(deserialize_with = "odoo::text", default)]
    pub date_deadline: Option<String>,
    #[serde(deserialize_with = "odoo::many2many_ids", default)]
    pub tag_ids: Vec<i64>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(rename = "type", deserialize_with = "odoo::text", default)]
    pub kind: Option<String>,
}

/// One `crm.tag` as `read` returns it — the join table for [`LeadRow::tags`].
#[derive(Debug, Clone, serde::Deserialize)]
pub struct TagRow {
    pub id: i64,
    pub name: String,
}

/// The outcome of `crm_lead_delete`: the id that was unlinked and the name it
/// carried, read before deletion so the summary can still say what is gone.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LeadDeleted {
    pub id: i64,
    pub name: Option<String>,
    pub deleted: bool,
}

pub use crate::shape as odoo;

// Why: dashboards consume this as structured rows — a table artifact, not
// prose. The markdown row below stays for the model-facing text rendering;
// machines must never have to regex it back apart.
#[doc(hidden)]
#[must_use]
pub fn lead_rows(records: &[serde_json::Value]) -> Vec<LeadRow> {
    // JSON: protocol boundary — records arrive as the RPC client's JSON. A
    // record that fails to type is logged and dropped rather than shipped
    // half-parsed.
    records
        .iter()
        .filter_map(
            |record| match serde_json::from_value::<LeadRow>(record.clone()) {
                Ok(row) => Some(row),
                Err(e) => {
                    tracing::warn!(error = %e, "crm.lead record did not match LeadRow; dropping");
                    None
                },
            },
        )
        .collect()
}

#[doc(hidden)]
#[must_use]
pub fn tag_ids_of(rows: &[LeadRow]) -> Vec<i64> {
    let mut ids: Vec<i64> = rows
        .iter()
        .flat_map(|r| r.tag_ids.iter().copied())
        .collect();
    ids.sort_unstable();
    ids.dedup();
    ids
}

#[doc(hidden)]
pub fn attach_tag_names<S: std::hash::BuildHasher>(
    rows: &mut [LeadRow],
    names: &HashMap<i64, String, S>,
) {
    for row in rows {
        row.tags = row
            .tag_ids
            .iter()
            .filter_map(|id| names.get(id).cloned())
            .collect();
    }
}

#[doc(hidden)]
#[must_use]
pub fn tag_names(tags: &[serde_json::Value]) -> HashMap<i64, String> {
    // JSON: protocol boundary — `crm.tag` rows as the RPC client returns them.
    tags.iter()
        .filter_map(|t| match serde_json::from_value::<TagRow>(t.clone()) {
            Ok(tag) => Some((tag.id, tag.name)),
            Err(e) => {
                tracing::warn!(error = %e, "crm.tag record did not match TagRow; dropping");
                None
            },
        })
        .collect()
}

#[doc(hidden)]
#[must_use]
pub fn lead_table(rows: &[LeadRow]) -> TableArtifact {
    // Why: this carried eleven columns, of which about five fitted the chat
    // column — email, revenue, probability, dates and tags sat past the scroll
    // edge, invisible unless the reader dragged the table sideways, while
    // widening every visible cell to make room for them. These six are what a
    // reader scans; the full record is one `crm_lead_get` away.
    let columns = vec![
        Column::new("id", ColumnType::Integer),
        Column::new("name", ColumnType::String).with_header("Subject"),
        Column::new("stage_id", ColumnType::String).with_header("Stage"),
        Column::new("user_id", ColumnType::String).with_header("Salesperson"),
        Column::new("partner_name", ColumnType::String).with_header("Contact"),
        Column::new("expected_revenue", ColumnType::Currency).with_header("Expected revenue"),
    ];
    // JSON: protocol boundary — TableArtifact carries rows as JSON values.
    let items = rows
        .iter()
        .filter_map(|row| match serde_json::to_value(row) {
            Ok(item) => Some(item),
            Err(e) => {
                tracing::warn!(error = %e, lead_id = row.id, "lead row did not serialise; dropping");
                None
            },
        })
        .collect();
    // Why: A search over an open pipeline routinely returns forty-odd leads.
    // Without a page size every one of them renders, so the artifact grows with
    // the pipeline; with one it is a fixed-height component the reader can sort
    // and filter in place.
    TableArtifact::new(columns)
        .with_title("CRM Leads")
        .with_rows(items)
        .with_hints(
            TableHints::new()
                .with_page_size(8)
                .filterable()
                .with_sortable(vec![
                    "id".to_owned(),
                    "name".to_owned(),
                    "stage_id".to_owned(),
                    "user_id".to_owned(),
                    "expected_revenue".to_owned(),
                ]),
        )
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
