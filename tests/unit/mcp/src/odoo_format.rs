//! Rendering Odoo records.
//!
//! Odoo returns `false` for an empty field and `[id, "Name"]` for a relation.
//! Passing either through unfiltered puts "false" in front of a model as if it
//! were data.

use systemprompt_mcp_odoo::format::{
    detail_lines, empty_result, field, field_or_dash, relation_id,
};
use systemprompt_mcp_odoo::server::crm::lead_row;
use systemprompt_mcp_odoo::server::report::group_row;

fn lead() -> serde_json::Value {
    serde_json::json!({
        "id": 12,
        "name": "Acme rollout",
        "partner_name": "Acme Ltd",
        "email_from": false,
        "phone": "",
        "stage_id": [3, "Qualified"],
        "user_id": [7, "Jo Salesperson"],
        "expected_revenue": 25000.0,
    })
}

#[test]
fn field_reads_a_plain_string() {
    assert_eq!(field(&lead(), "name").as_deref(), Some("Acme rollout"));
}

#[test]
fn field_treats_odoos_false_as_absent() {
    assert_eq!(
        field(&lead(), "email_from"),
        None,
        "Odoo writes `false`, not null, for an empty field"
    );
}

#[test]
fn field_treats_an_empty_string_as_absent() {
    assert_eq!(field(&lead(), "phone"), None);
}

#[test]
fn field_reads_the_display_name_out_of_a_relation() {
    assert_eq!(
        field(&lead(), "stage_id").as_deref(),
        Some("Qualified"),
        "a many2one is [id, name]; the name is what a reader wants"
    );
}

#[test]
fn relation_id_reads_the_id_side_for_follow_up_calls() {
    assert_eq!(relation_id(&lead(), "stage_id"), Some(3));
    assert_eq!(relation_id(&lead(), "name"), None);
}

#[test]
fn field_or_dash_fills_a_missing_column() {
    assert_eq!(field_or_dash(&lead(), "email_from"), "—");
    assert_eq!(field_or_dash(&lead(), "absent_field"), "—");
}

#[test]
fn detail_lines_skips_absent_fields_rather_than_printing_blanks() {
    let rendered = detail_lines(&lead(), &[("name", "Subject"), ("email_from", "Email")]);

    assert_eq!(rendered, "- **Subject:** Acme rollout");
}

#[test]
fn lead_row_leads_with_the_id_so_a_follow_up_call_is_possible() {
    let row = lead_row(&lead());

    assert!(row.starts_with("- **[12] Acme rollout**"), "got: {row}");
    assert!(row.contains("Qualified"));
    assert!(row.contains("Jo Salesperson"));
}

#[test]
fn group_row_reads_odoos_aggregate_column_names() {
    let bucket = serde_json::json!({
        "stage_id": [3, "Qualified"],
        "__count": 4,
        "expected_revenue": 100_000.0,
    });

    let row = group_row(&bucket, "stage_id");

    assert_eq!(
        row, "- **Qualified** — 4 lead(s), expected revenue 100000.00",
        "read_group names the count `__count` and the sum after the field itself"
    );
}

#[test]
fn empty_result_says_the_query_succeeded() {
    let message = empty_result("leads");

    assert!(
        message.contains("not an error"),
        "a model must not retry a query that ran fine and matched nothing: {message}"
    );
}
