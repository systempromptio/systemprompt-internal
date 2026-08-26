//! Odoo group → platform role mapping.
//!
//! The properties worth pinning are the safety directions: an unmapped group
//! grants nothing, the defaults always apply, and a malformed RPC row is
//! skipped rather than turned into a half-formed xml_id — because whatever
//! comes out of this mapping is written straight onto `users.roles`.

use systemprompt_web_admin::test_support::OdooRoleMap;

fn mapping(yaml: &str) -> OdooRoleMap {
    serde_yaml::from_str(yaml).expect("test mapping parses")
}

#[test]
fn an_admin_group_grants_admin_on_top_of_the_defaults() {
    let map = mapping("default_roles: [user]\ngroups:\n  base.group_system: [admin]\n");

    assert_eq!(
        map.roles_for(&[
            "base.group_system".to_owned(),
            "sales_team.group_sale_salesman".to_owned()
        ]),
        vec!["admin".to_owned(), "user".to_owned()],
        "unmapped groups grant nothing; mapped ones add to the defaults"
    );
}

#[test]
fn no_matching_group_means_defaults_only() {
    let map = mapping("default_roles: [user]\ngroups:\n  base.group_system: [admin]\n");

    assert_eq!(
        map.roles_for(&["base.group_user".to_owned()]),
        vec!["user".to_owned()],
        "a plain employee must never come out of this mapping with admin"
    );
}

#[test]
fn duplicate_grants_collapse_to_one_role() {
    let map = mapping("default_roles: [user]\ngroups:\n  a.one: [admin, user]\n  a.two: [admin]\n");

    assert_eq!(
        map.roles_for(&["a.one".to_owned(), "a.two".to_owned()]),
        vec!["admin".to_owned(), "user".to_owned()],
        "users.roles is a set; duplicates would leak into every JWT"
    );
}

#[test]
fn an_empty_mapping_file_still_yields_a_valid_empty_set() {
    let map = mapping("{}");

    assert!(map.roles_for(&["base.group_system".to_owned()]).is_empty());
}
