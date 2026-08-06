//! The chatter tools' domains and rendering.
//!
//! `note_search` is the knowledge bank's retrieval path, and its domain has the
//! property most easily got wrong in prefix notation: the two text leaves must
//! be an OR that the filters then AND onto. Written wrong, it silently returns
//! subject-only matches or drops the model filter — no error, just wrong
//! answers.

use systemprompt_mcp_odoo::server::notes::{
    search_domain, search_row, thread_domain, thread_row,
};
use systemprompt_mcp_odoo::tools::inputs::NoteSearchInput;

fn search(query: &str, model: Option<&str>, from: Option<&str>, to: Option<&str>) -> NoteSearchInput {
    NoteSearchInput {
        query: query.to_owned(),
        model: model.map(str::to_owned),
        date_from: from.map(str::to_owned),
        date_to: to.map(str::to_owned),
        limit: None,
    }
}

#[test]
fn thread_domain_anchors_on_both_halves_of_the_record_reference() {
    assert_eq!(
        thread_domain("crm.lead", 42),
        serde_json::json!([["model", "=", "crm.lead"], ["res_id", "=", 42]]),
        "an id without its model would match another model's record 42"
    );
}

#[test]
fn search_domain_ors_body_and_subject() {
    assert_eq!(
        search_domain(&search("pricing", None, None, None)),
        serde_json::json!([
            "|",
            ["body", "ilike", "%pricing%"],
            ["subject", "ilike", "%pricing%"]
        ]),
        "Odoo puts an emailed note's text in body and its heading in subject"
    );
}

#[test]
fn search_domain_wraps_the_query_in_wildcards() {
    let domain = search_domain(&search("pricing", None, None, None));

    assert_eq!(
        domain[1][2], "%pricing%",
        "a bare ilike without wildcards is an equality match in disguise"
    );
}

#[test]
fn search_domain_trims_the_query_before_wildcarding() {
    let domain = search_domain(&search("  pricing  ", None, None, None));

    assert_eq!(
        domain[1][2], "%pricing%",
        "leading whitespace inside the wildcards would never match"
    );
}

#[test]
fn the_model_filter_ands_onto_the_text_group() {
    let domain = search_domain(&search("pricing", Some("crm.lead"), None, None));
    let leaves = domain.as_array().expect("a domain is an array");

    assert_eq!(leaves[0], "|", "the OR still governs only the two text leaves");
    assert_eq!(
        leaves.last(),
        Some(&serde_json::json!(["model", "=", "crm.lead"])),
        "a bare leaf after the OR group is an implicit AND"
    );
}

#[test]
fn both_date_bounds_are_applied() {
    let domain = search_domain(&search("x", None, Some("2026-01-01"), Some("2026-06-30")));
    let leaves = domain.as_array().expect("a domain is an array");

    assert!(leaves.contains(&serde_json::json!(["date", ">=", "2026-01-01"])));
    assert!(leaves.contains(&serde_json::json!(["date", "<=", "2026-06-30"])));
}

#[test]
fn blank_filters_are_ignored_rather_than_matched_on() {
    let domain = search_domain(&search("x", Some("   "), Some(""), None));

    assert_eq!(
        domain.as_array().map(Vec::len),
        Some(3),
        "whitespace is an omitted filter, not a filter for whitespace: {domain}"
    );
}

#[test]
fn a_thread_row_renders_the_body_as_text_not_html() {
    let record = serde_json::json!({
        "id": 9,
        "date": "2026-08-01 09:00:00",
        "author_id": [4, "Jo Salesperson"],
        "message_type": "comment",
        "body": "<p>Customer wants <b>net 60</b></p>",
    });

    let row = thread_row(&record);

    assert!(row.contains("Customer wants net 60"), "got: {row}");
    assert!(!row.contains("<p>"), "markup must not reach the model: {row}");
    assert!(row.contains("Jo Salesperson") && row.contains("2026-08-01 09:00:00"));
}

#[test]
fn an_empty_body_is_named_rather_than_rendered_blank() {
    let record = serde_json::json!({ "id": 9, "body": false });

    assert!(
        thread_row(&record).contains("(empty note)"),
        "Odoo writes false for an empty body; a blank line would look like a bug"
    );
}

#[test]
fn a_search_row_carries_the_anchor_needed_to_follow_it() {
    let record = serde_json::json!({
        "id": 9,
        "model": "crm.lead",
        "res_id": 42,
        "record_name": "Acme rollout",
        "author_id": [4, "Jo Salesperson"],
        "date": "2026-08-01 09:00:00",
        "body": "<p>They asked about pricing tiers</p>",
    });

    let row = search_row(&record, "pricing");

    assert!(row.contains("crm.lead"), "the model is half the anchor: {row}");
    assert!(row.contains("42"), "the id is the other half: {row}");
    assert!(row.contains("Acme rollout"));
    assert!(row.contains("pricing tiers"), "the snippet shows the match: {row}");
}

#[test]
fn a_search_row_snippet_is_plain_text() {
    let record = serde_json::json!({
        "model": "crm.lead",
        "res_id": 1,
        "body": "<div><p>alpha</p><p>beta</p></div>",
    });

    let row = search_row(&record, "beta");

    assert!(row.contains("alpha beta"), "got: {row}");
    assert!(!row.contains('<'), "no markup survives into a snippet: {row}");
}
