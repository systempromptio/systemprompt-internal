//! The Odoo search domains the tools build.
//!
//! A wrong domain does not fail — it returns the wrong records, which is worse.
//! These assertions pin the prefix-notation operator placement and, for
//! activities, the acting-uid filter that makes "list my activities" mean the
//! caller's.

use systemprompt_mcp_odoo::server::activity::activity_domain;
use systemprompt_mcp_odoo::server::crm::lead_domain;
use systemprompt_mcp_odoo::server::partner::partner_domain;
use systemprompt_mcp_odoo::server::report::report_domain;
use systemprompt_mcp_odoo::tools::inputs::{
    ActivityListInput, LeadReportInput, LeadSearchInput, ReportGroupBy, resolve_limit,
};

fn lead_input(query: Option<&str>, stage: Option<&str>, user: Option<&str>) -> LeadSearchInput {
    LeadSearchInput {
        query: query.map(str::to_owned),
        stage: stage.map(str::to_owned),
        user: user.map(str::to_owned),
        limit: None,
    }
}

#[test]
fn lead_domain_is_empty_when_nothing_is_filtered() {
    assert_eq!(
        lead_domain(&lead_input(None, None, None)),
        serde_json::json!([]),
        "an unfiltered search must not smuggle in a condition"
    );
}

#[test]
fn lead_domain_ors_the_free_text_across_three_columns() {
    let domain = lead_domain(&lead_input(Some("acme"), None, None));

    assert_eq!(
        domain,
        serde_json::json!([
            "|",
            "|",
            ["name", "ilike", "acme"],
            ["partner_name", "ilike", "acme"],
            ["email_from", "ilike", "acme"]
        ]),
        "two '|' prefixes are what make three leaves a single OR group"
    );
}

#[test]
fn lead_domain_ands_the_stage_filter_onto_the_text_group() {
    let domain = lead_domain(&lead_input(Some("acme"), Some("Qualified"), None));
    let leaves = domain.as_array().expect("a domain is an array");

    assert_eq!(
        leaves.last(),
        Some(&serde_json::json!(["stage_id.name", "ilike", "Qualified"])),
        "a bare leaf appended after an OR group is an implicit AND"
    );
}

#[test]
fn lead_domain_matches_a_salesperson_by_name_or_login() {
    let domain = lead_domain(&lead_input(None, None, Some("jo")));

    assert_eq!(
        domain,
        serde_json::json!([
            "|",
            ["user_id.name", "ilike", "jo"],
            ["user_id.login", "ilike", "jo"]
        ]),
        "callers say \"assigned to jo\" without knowing which of the two they mean"
    );
}

#[test]
fn lead_domain_ignores_blank_filters() {
    assert_eq!(
        lead_domain(&lead_input(Some("   "), Some(""), None)),
        serde_json::json!([]),
        "whitespace is an omitted filter, not a search for whitespace"
    );
}

#[test]
fn partner_domain_ors_name_email_and_phone() {
    assert_eq!(
        partner_domain("acme"),
        serde_json::json!([
            "|",
            "|",
            ["name", "ilike", "acme"],
            ["email", "ilike", "acme"],
            ["phone", "ilike", "acme"]
        ])
    );
}

#[test]
fn activity_domain_always_pins_the_acting_user() {
    let input = ActivityListInput {
        model: None,
        overdue_only: None,
        limit: None,
    };

    let domain = activity_domain(42, &input, "2026-08-06");

    assert_eq!(
        domain,
        serde_json::json!([["user_id", "=", 42]]),
        "the uid filter is the tool's contract — \"my activities\" cannot mean anyone else's"
    );
}

#[test]
fn activity_domain_adds_model_and_overdue_filters() {
    let input = ActivityListInput {
        model: Some("crm.lead".to_owned()),
        overdue_only: Some(true),
        limit: None,
    };

    let domain = activity_domain(42, &input, "2026-08-06");

    assert_eq!(
        domain,
        serde_json::json!([
            ["user_id", "=", 42],
            ["res_model", "=", "crm.lead"],
            ["date_deadline", "<", "2026-08-06"]
        ]),
        "overdue is strictly before today; what is due today is not yet late"
    );
}

#[test]
fn report_domain_covers_all_four_date_window_combinations() {
    let base = |from: Option<&str>, to: Option<&str>| LeadReportInput {
        group_by: ReportGroupBy::Stage,
        date_from: from.map(str::to_owned),
        date_to: to.map(str::to_owned),
    };

    assert_eq!(report_domain(&base(None, None)), serde_json::json!([]));
    assert_eq!(
        report_domain(&base(Some("2026-01-01"), None)),
        serde_json::json!([["create_date", ">=", "2026-01-01"]])
    );
    assert_eq!(
        report_domain(&base(None, Some("2026-12-31"))),
        serde_json::json!([["create_date", "<=", "2026-12-31"]])
    );
    assert_eq!(
        report_domain(&base(Some("2026-01-01"), Some("2026-12-31"))),
        serde_json::json!([
            ["create_date", ">=", "2026-01-01"],
            ["create_date", "<=", "2026-12-31"]
        ])
    );
}

#[test]
fn resolve_limit_defaults_and_caps() {
    assert_eq!(resolve_limit(None), 20);
    assert_eq!(resolve_limit(Some(5)), 5);
    assert_eq!(
        resolve_limit(Some(0)),
        1,
        "zero would return nothing at all"
    );
    assert_eq!(
        resolve_limit(Some(5000)),
        100,
        "a model asking for everything gets a page, not a context overflow"
    );
}
