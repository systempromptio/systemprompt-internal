//! `repositories::users` — identity CRUD, role and membership lookup, the index
//! listing, and the per-user side tables.

use systemprompt::identifiers::{Email, UserId};
use systemprompt_web_admin::repositories::users;
use systemprompt_web_admin::types::{CreateUserRequest, UpdateUserRequest};

use crate::fixtures::{
    insert_group, insert_group_member, insert_project, insert_project_member, insert_user,
    insert_user_full, unclaimed_email, unique, unique_group, unique_project,
};
use crate::tempdb::TempDb;

fn create_request(user_id: &str, email: &str) -> CreateUserRequest {
    CreateUserRequest {
        user_id: UserId::new(user_id.to_owned()),
        display_name: "Fixture User".to_owned(),
        email: Email::try_new(email.to_owned()).expect("fixture email is valid"),
        roles: vec!["user".to_owned()],
        status: None,
    }
}

fn empty_update() -> UpdateUserRequest {
    UpdateUserRequest {
        display_name: None,
        email: None,
        is_active: None,
    }
}

#[tokio::test]
async fn create_user_returns_the_row_it_inserted() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let id = unique("user");
    let email = unclaimed_email("created");

    let summary = users::create_user(&db.pool, &create_request(&id, &email))
        .await
        .expect("create_user succeeds for an unclaimed domain");

    assert_eq!(summary.user_id.as_str(), id);
    assert_eq!(summary.display_name.as_deref(), Some("Fixture User"));
    assert!(summary.is_active, "no status given defaults to active");
    assert_eq!(summary.roles, vec!["user".to_owned()]);
    db.cleanup().await;
}

#[tokio::test]
async fn create_user_honours_an_explicit_inactive_status() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let mut req = create_request(&unique("user"), &unclaimed_email("inactive"));
    req.status = Some("inactive".to_owned());

    let summary = users::create_user(&db.pool, &req)
        .await
        .expect("create_user succeeds");

    assert!(!summary.is_active);
    db.cleanup().await;
}

#[tokio::test]
async fn create_user_on_a_taken_email_updates_rather_than_duplicating() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("conflict");
    let first = unique("user");
    users::create_user(&db.pool, &create_request(&first, &email))
        .await
        .expect("first create succeeds");

    let mut second = create_request(&unique("user"), &email);
    second.roles = vec!["user".to_owned(), "admin".to_owned()];
    let summary = users::create_user(&db.pool, &second)
        .await
        .expect("ON CONFLICT (email) updates the existing row");

    assert_eq!(
        summary.user_id.as_str(),
        first,
        "the conflicting insert must keep the original row's id"
    );
    assert!(summary.roles.contains(&"admin".to_owned()));
    db.cleanup().await;
}

#[tokio::test]
async fn update_user_returns_none_for_an_absent_user() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let missing = UserId::new(unique("absent"));

    let updated = users::update_user(&db.pool, &missing, &empty_update())
        .await
        .expect("update of an absent user is not an error");

    assert!(
        updated.is_none(),
        "update_ of an absent row reports None, not an error"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn update_user_renames_the_display_name() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("rename")).await;
    let mut req = empty_update();
    req.display_name = Some("Renamed".to_owned());

    let updated = users::update_user(&db.pool, &user, &req)
        .await
        .expect("update succeeds")
        .expect("an existing user yields a row");

    assert_eq!(updated.display_name.as_deref(), Some("Renamed"));
    db.cleanup().await;
}

#[tokio::test]
async fn update_user_deactivating_flips_is_active() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("deactivate")).await;
    let mut req = empty_update();
    req.is_active = Some(false);

    let updated = users::update_user(&db.pool, &user, &req)
        .await
        .expect("update succeeds")
        .expect("an existing user yields a row");

    assert!(!updated.is_active);
    db.cleanup().await;
}

#[tokio::test]
async fn update_user_leaves_fields_the_request_omits() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("partial");
    let user = insert_user(&db.pool, &unique("user"), &email).await;
    let mut req = empty_update();
    req.display_name = Some("Renamed Person".to_owned());

    let updated = users::update_user(&db.pool, &user, &req)
        .await
        .expect("update succeeds")
        .expect("an existing user yields a row");

    assert_eq!(updated.display_name.as_deref(), Some("Renamed Person"));
    assert_eq!(
        updated.email.as_ref().map(Email::as_str),
        Some(email.as_str()),
        "a None email must not clear the stored address"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn delete_user_reports_whether_a_row_went() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("delete")).await;

    let removed = users::delete_user(&db.pool, &user)
        .await
        .expect("delete succeeds");
    let removed_again = users::delete_user(&db.pool, &user)
        .await
        .expect("a second delete is not an error");

    assert!(removed, "the first delete removes the row");
    assert!(!removed_again, "the second finds nothing to remove");
    db.cleanup().await;
}

#[tokio::test]
async fn find_user_access_profile_returns_none_for_an_absent_user() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let missing = UserId::new(unique("absent"));

    let found = users::queries::find_user_access_profile(&db.pool, &missing)
        .await
        .expect("lookup succeeds");

    assert!(
        found.is_none(),
        "find_ reports absence as None, not an error"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn find_user_access_profile_reads_the_roles_and_the_derived_group() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user_full(
        &db.pool,
        &unique("user"),
        &unclaimed_email("noprofile"),
        Some("No Profile"),
        &["user".to_owned(), "auditor".to_owned()],
        "active",
    )
    .await;

    let profile = users::queries::find_user_access_profile(&db.pool, &user)
        .await
        .expect("lookup succeeds")
        .expect("the user exists");

    assert_eq!(profile.roles, vec!["user".to_owned(), "auditor".to_owned()]);
    assert_eq!(
        profile.group_ids,
        vec!["unassigned".to_owned()],
        "no membership row is the derived group, not an empty list"
    );
    assert!(
        profile.project_ids.is_empty(),
        "work attribution is optional"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn find_user_access_profile_reads_an_assigned_group_and_project() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("assigned")).await;
    let group = unique_group("grp");
    let project = unique_project("proj");
    insert_group(&db.pool, &group, "Commerce").await;
    insert_group_member(&db.pool, &group, &user, "adfs").await;
    insert_project(&db.pool, &project, "Commerce").await;
    insert_project_member(&db.pool, &project, &user, "adfs").await;

    let profile = users::queries::find_user_access_profile(&db.pool, &user)
        .await
        .expect("lookup succeeds")
        .expect("the user exists");

    assert_eq!(profile.group_ids, vec![group.as_str().to_owned()]);
    assert_eq!(profile.project_ids, vec![project.as_str().to_owned()]);
    db.cleanup().await;
}
