//! `repositories::users::access_control::matrix` — the extension subject
//! dimensions and the multi-section shape of the grid.
//!
//! `group` and `project` are not core concepts: they reach the resolver only
//! because this extension registers a `SubjectAttributeProvider` for each. A
//! rule written against one therefore proves the whole registration path, not
//! just the SQL, and the two together pin the 140/150 half of the ladder
//! against the 160/170 link bands. The remaining tests cover what the grid
//! does with more than one section and with an entity type core's
//! `EntityKind` does not know.

use systemprompt_web_admin::repositories::users::access_control::{
    filter_catalog_for_user, resolve_user_matrix,
};

use crate::fixtures::{
    insert_acl_rule, insert_group, insert_group_member, insert_project, insert_project_member,
    insert_user, unclaimed_email, unique, unique_group, unique_project,
};
use crate::tempdb::TempDb;
use crate::users_access_matrix::{grade, one_skill};

#[tokio::test]
async fn resolve_user_matrix_binds_a_group_rule_through_the_extension_dimension() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("grouprule")).await;
    let group = unique_group("grp");
    insert_group(&db.pool, &group, "Commerce").await;
    insert_group_member(&db.pool, &group, &user, "adfs").await;
    let skill = unique("skill");
    insert_acl_rule(&db.pool, "skill", &skill, "group", group.as_str(), "allow").await;

    let row = grade(&db.pool, &user, &skill).await;

    assert_eq!(row.effective, "allow");
    assert_eq!(row.source.layer, "group");
    db.cleanup().await;
}

#[tokio::test]
async fn resolve_user_matrix_does_not_bind_a_rule_for_a_group_the_user_left() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("leftgroup")).await;
    let held = unique_group("grp");
    let granted = unique_group("grp");
    insert_group(&db.pool, &held, "Core").await;
    insert_group(&db.pool, &granted, "Commerce").await;
    insert_group_member(&db.pool, &held, &user, "adfs").await;
    let skill = unique("skill");
    insert_acl_rule(
        &db.pool,
        "skill",
        &skill,
        "group",
        granted.as_str(),
        "allow",
    )
    .await;

    let row = grade(&db.pool, &user, &skill).await;

    assert_eq!(row.effective, "deny");
    db.cleanup().await;
}

// Why: the four extension bands must all reach the grid, and `project` (140)
// must out-rank `group` (150) when both match the same entity. A ladder that
// only ever resolves one band is indistinguishable from a broken one.
#[tokio::test]
async fn a_project_rule_out_ranks_a_group_rule_on_the_same_entity() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("ladder")).await;
    let group = unique_group("grp");
    let project = unique_project("proj");
    insert_group(&db.pool, &group, "Commerce").await;
    insert_group_member(&db.pool, &group, &user, "adfs").await;
    insert_project(&db.pool, &project, "Storefront").await;
    insert_project_member(&db.pool, &project, &user, "adfs").await;
    let skill = unique("skill");
    insert_acl_rule(&db.pool, "skill", &skill, "group", group.as_str(), "allow").await;
    insert_acl_rule(
        &db.pool,
        "skill",
        &skill,
        "project",
        project.as_str(),
        "deny",
    )
    .await;

    let row = grade(&db.pool, &user, &skill).await;

    assert_eq!(row.effective, "deny");
    assert_eq!(row.source.layer, "project", "the narrower band decides");
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
