//! The typed lead row and its table projection.
//!
//! Odoo's wire idioms — `false` for an empty field, `[id, "Name"]` for a
//! many2one — must be absorbed at deserialization, because whatever comes out
//! of `lead_table` is the structured contract the dashboards render verbatim.

use systemprompt_mcp_odoo::server::crm::{LeadRow, lead_table};

fn record() -> serde_json::Value {
    serde_json::json!({
        "id": 47,
        "name": "Acme rollout",
        "partner_name": "Acme Corp",
        "email_from": false,
        "phone": false,
        "stage_id": [1, "New"],
        "user_id": [7, "Jo Salesperson"],
        "expected_revenue": 40000.0,
        "probability": false,
        "create_date": "2026-08-26 09:00:00",
    })
}

#[test]
fn a_lead_record_types_cleanly_including_odoo_false_and_many2one() {
    let row: LeadRow = serde_json::from_value(record()).expect("record types");

    assert_eq!(row.id, 47);
    assert_eq!(row.stage_id.as_deref(), Some("New"), "many2one → name");
    assert_eq!(row.user_id.as_deref(), Some("Jo Salesperson"));
    assert_eq!(row.email_from, None, "Odoo's false means absent, not \"false\"");
    assert_eq!(row.probability, None);
    assert_eq!(row.expected_revenue, Some(40000.0));
}

#[test]
fn the_table_carries_odoo_field_names_the_dashboards_key_on() {
    let table = lead_table(&[record()]);
    let value = serde_json::to_value(&table).expect("serializes");

    let names: Vec<&str> = value["columns"]
        .as_array()
        .expect("columns")
        .iter()
        .filter_map(|c| c["name"].as_str())
        .collect();
    assert_eq!(
        names,
        vec![
            "id",
            "name",
            "stage_id",
            "user_id",
            "partner_name",
            "email_from",
            "expected_revenue",
            "probability",
            "create_date"
        ],
        "renaming a column silently breaks every dashboard keyed on these"
    );
    assert_eq!(value["items"][0]["stage_id"], "New");
    assert_eq!(value["items"][0]["expected_revenue"], 40000.0);
}

#[test]
fn a_malformed_record_is_dropped_not_shipped_half_parsed() {
    let table = lead_table(&[record(), serde_json::json!({ "name": "no id" })]);
    let value = serde_json::to_value(&table).expect("serializes");

    assert_eq!(
        value["items"].as_array().map(Vec::len),
        Some(1),
        "the well-formed row survives; the id-less one must not become a phantom lead"
    );
}
