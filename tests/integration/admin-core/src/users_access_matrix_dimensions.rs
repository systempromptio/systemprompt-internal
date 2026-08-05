//! `repositories::users::access_control::matrix` — the extension subject
//! dimensions and the multi-section shape of the grid.
//!
//! `department` and `organization` are not core concepts: they reach the
//! resolver only because this extension registers a `SubjectAttributeProvider`
//! for each. A rule written against either therefore proves the whole
//! registration path, not just the SQL. The remaining tests cover what the
//! grid does with more than one section and with an entity type core's
//! `EntityKind` does not know.

use systemprompt_web_admin::repositories::users::access_control::{
    filter_catalog_for_user, resolve_user_matrix,
};

use crate::fixtures::{
    OrgSpec, insert_acl_rule, insert_member, insert_org, insert_user, set_department,
    unclaimed_email, unique,
};
use crate::tempdb::TempDb;
use crate::users_access_matrix::{grade, one_skill};

#[tokio::test]
async fn resolve_user_matrix_binds_a_department_rule_through_the_extension_dimension() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("deptrule")).await;
    set_department(&db.pool, &user, "Platform").await;
    let skill = unique("skill");
    insert_acl_rule(&db.pool, "skill", &skill, "department", "Platform", "allow").await;

    let row = grade(&db.pool, &user, &skill).await;

    assert_eq!(row.effective, "allow");
    assert_eq!(row.source.layer, "department");
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_user_matrix_binds_an_organization_rule_through_the_extension_dimension() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("orgrule")).await;
    let org_id = unique("org");
    let slug = unique("slug");
    insert_org(&db.pool, &OrgSpec::active(&org_id, &slug)).await;
    insert_member(&db.pool, &user, &org_id, "member").await;
    let skill = unique("skill");
    // The organization dimension resolves to the org *slug*, not its id.
    insert_acl_rule(&db.pool, "skill", &skill, "organization", &slug, "allow").await;

    let row = grade(&db.pool, &user, &skill).await;

    assert_eq!(row.effective, "allow");
    assert_eq!(row.source.layer, "organization");
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_user_matrix_falls_back_to_the_default_for_an_unrecognised_entity_type() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("badkind")).await;
    let sections = vec![(
        "warp_drive".to_owned(),
        "Warp Drives".to_owned(),
        vec![(unique("entity"), "A Drive".to_owned(), None)],
    )];

    let matrix = resolve_user_matrix(&db.pool, &user, sections)
        .await
        .expect("resolve matrix")
        .expect("user found");

    let row = &matrix.sections[0].rows[0];
    assert_eq!(row.effective, "deny");
    assert_eq!(row.source.layer, "default");
    assert!(row.source.detail.contains("unknown entity type"));
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_user_matrix_grades_every_row_of_every_section() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("sections")).await;
    let allowed = unique("skill");
    insert_acl_rule(&db.pool, "skill", &allowed, "user", user.as_str(), "allow").await;
    let server = unique("server");
    let sections = vec![
        (
            "skill".to_owned(),
            "Skills".to_owned(),
            vec![
                (
                    allowed.clone(),
                    "Allowed".to_owned(),
                    Some("desc".to_owned()),
                ),
                (unique("skill"), "Blocked".to_owned(), None),
            ],
        ),
        (
            "mcp_server".to_owned(),
            "MCP Servers".to_owned(),
            vec![(server, "A Server".to_owned(), None)],
        ),
    ];

    let matrix = resolve_user_matrix(&db.pool, &user, sections)
        .await
        .expect("resolve matrix")
        .expect("user found");

    assert_eq!(matrix.sections.len(), 2);
    assert_eq!(matrix.sections[0].label, "Skills");
    assert_eq!(matrix.sections[0].rows.len(), 2);
    assert_eq!(matrix.sections[0].rows[0].effective, "allow");
    assert_eq!(
        matrix.sections[0].rows[0].description.as_deref(),
        Some("desc")
    );
    assert_eq!(matrix.sections[0].rows[1].effective, "deny");
    assert_eq!(matrix.sections[1].entity_type, "mcp_server");
    assert_eq!(matrix.sections[1].rows[0].effective, "deny");
    db.cleanup().await;
}

#[tokio::test]
async fn filter_catalog_for_user_is_the_same_grading_as_resolve_user_matrix() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("filter")).await;
    let skill = unique("skill");
    insert_acl_rule(&db.pool, "skill", &skill, "user", user.as_str(), "allow").await;

    let matrix = filter_catalog_for_user(&db.pool, &user, one_skill(&skill))
        .await
        .expect("filter catalog")
        .expect("user found");

    assert_eq!(matrix.sections[0].rows[0].effective, "allow");
    assert_eq!(matrix.sections[0].rows[0].source.layer, "user");
    db.cleanup().await;
}
