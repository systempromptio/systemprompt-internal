//! The typed lead row and its table projection.
//!
//! Odoo's wire idioms — `false` for an empty field, `[id, "Name"]` for a
//! many2one, a bare id list for a many2many — must be absorbed at
//! deserialization, because whatever comes out of `lead_table` is the
//! structured contract the dashboards render verbatim.

use std::collections::HashMap;

use systemprompt_mcp_odoo::server::crm::{
    LeadDeleted, LeadRow, attach_tag_names, lead_rows, lead_table, tag_ids_of, tag_names,
};

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
        "date_deadline": "2026-09-30",
        "tag_ids": [3, 5],
        "type": "opportunity",
    })
}

fn rows(records: &[serde_json::Value]) -> Vec<LeadRow> {
    lead_rows(records)
}

#[test]
fn a_lead_record_types_cleanly_including_odoo_false_and_many2one() {
    let row: LeadRow = serde_json::from_value(record()).expect("record types");

    assert_eq!(row.id, 47);
    assert_eq!(row.stage_id.as_deref(), Some("New"), "many2one → name");
    assert_eq!(row.salesperson.as_deref(), Some("Jo Salesperson"));
    assert_eq!(
        row.email_from, None,
        "Odoo's false means absent, not \"false\""
    );
    assert_eq!(row.probability, None);
    assert_eq!(row.expected_revenue, Some(40000.0));
    assert_eq!(row.date_deadline.as_deref(), Some("2026-09-30"));
    assert_eq!(row.tag_ids, vec![3, 5], "many2many → bare ids");
    assert!(
        row.tags.is_empty(),
        "names are attached server-side, never guessed off the wire"
    );
    assert_eq!(row.kind.as_deref(), Some("opportunity"));
}

#[test]
fn an_empty_many2many_arrives_as_false_and_types_to_no_ids() {
    let mut record = record();
    record["tag_ids"] = serde_json::json!(false);
    record["date_deadline"] = serde_json::json!(false);

    let row: LeadRow = serde_json::from_value(record).expect("record types");

    assert!(row.tag_ids.is_empty());
    assert_eq!(row.date_deadline, None);
}

#[test]
fn the_table_carries_odoo_field_names_the_dashboards_key_on() {
    let table = lead_table(&rows(&[record()]));
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
            "create_date",
            "date_deadline",
            "tags"
        ],
        "renaming a column silently breaks every dashboard keyed on these"
    );
    assert_eq!(value["items"][0]["stage_id"], "New");
    assert_eq!(value["items"][0]["expected_revenue"], 40000.0);
    assert_eq!(value["items"][0]["date_deadline"], "2026-09-30");
}

#[test]
fn tag_names_are_joined_onto_rows_by_id_and_unknown_ids_are_skipped() {
    let mut leads = rows(&[record()]);
    let names = tag_names(&[
        serde_json::json!({"id": 5, "name": "Legal"}),
        serde_json::json!({"id": 3, "name": "Sales"}),
        serde_json::json!({"name": "no id"}),
    ]);

    attach_tag_names(&mut leads, &names);

    assert_eq!(
        leads[0].tags,
        vec!["Sales".to_owned(), "Legal".to_owned()],
        "names follow the lead's own tag order, not the read order"
    );
    let value = serde_json::to_value(lead_table(&leads)).expect("serializes");
    assert_eq!(
        value["items"][0]["tags"],
        serde_json::json!(["Sales", "Legal"])
    );
}

#[test]
fn a_tag_id_with_no_name_is_dropped_rather_than_rendered_as_a_number() {
    let mut leads = rows(&[record()]);
    let names: HashMap<i64, String> = [(3, "Sales".to_owned())].into_iter().collect();

    attach_tag_names(&mut leads, &names);

    assert_eq!(leads[0].tags, vec!["Sales".to_owned()]);
}

#[test]
fn tag_ids_of_dedupes_across_rows_so_the_join_is_one_read() {
    let mut second = record();
    second["id"] = serde_json::json!(48);
    second["tag_ids"] = serde_json::json!([5, 9]);
    let leads = rows(&[
        record(),
        second,
        serde_json::json!({"id": 49, "tag_ids": false}),
    ]);

    assert_eq!(tag_ids_of(&leads), vec![3, 5, 9]);
}

#[test]
fn a_malformed_record_is_dropped_not_shipped_half_parsed() {
    let table = lead_table(&rows(&[record(), serde_json::json!({ "name": "no id" })]));
    let value = serde_json::to_value(&table).expect("serializes");

    assert_eq!(
        value["items"].as_array().map(Vec::len),
        Some(1),
        "the well-formed row survives; the id-less one must not become a phantom lead"
    );
}

#[test]
fn a_delete_precheck_record_types_into_lead_row_with_only_id_and_name() {
    let row: LeadRow =
        serde_json::from_value(serde_json::json!({"id": 47, "name": "Acme"})).expect("types");

    assert_eq!(row.id, 47);
    assert_eq!(row.name.as_deref(), Some("Acme"));
    assert!(row.stage_id.is_none());
}

#[test]
fn lead_deleted_serialises_id_name_and_deleted() {
    let deleted = LeadDeleted {
        id: 47,
        name: Some("Acme".to_owned()),
        deleted: true,
    };

    assert_eq!(
        serde_json::to_value(deleted).expect("serialises"),
        serde_json::json!({"id": 47, "name": "Acme", "deleted": true})
    );
}
