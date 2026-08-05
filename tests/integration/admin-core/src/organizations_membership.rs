//! `repositories::organizations::crud` — the organization index, membership,
//! the platform-tenant boundary, and the domain claim both provisioning doors
//! consult.
//!
//! The `house` organization exists in every database (migration 022) and is the
//! platform tenant (024), so the platform-boundary tests use it rather than
//! minting a second one — the partial unique index permits only one.

use systemprompt::identifiers::UserId;
use systemprompt_web_admin::repositories::organizations::crud;

use crate::fixtures::{
    OrgSpec, insert_member, insert_org, insert_plan, insert_user, insert_user_full,
    unclaimed_email, unique,
};
use crate::tempdb::TempDb;

const HOUSE: &str = "house";

#[tokio::test]
async fn list_organizations_reports_active_members_as_seats_used() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let org_id = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&org_id, &org_id)).await;
    let active = insert_user(&db.pool, &unique("user"), &unclaimed_email("active")).await;
    let dormant = insert_user_full(
        &db.pool,
        &unique("user"),
        &unclaimed_email("dormant"),
        None,
        &["user".to_owned()],
        "inactive",
    )
    .await;
    insert_member(&db.pool, &active, &org_id, "member").await;
    insert_member(&db.pool, &dormant, &org_id, "member").await;

    let orgs = crud::list_organizations(&db.pool)
        .await
        .expect("listing succeeds");

    let row = orgs
        .iter()
        .find(|o| o.id == org_id)
        .expect("the new organization is listed");
    assert_eq!(
        row.seats_used, 1,
        "a suspended member frees the seat without deleting their history"
    );
    assert_eq!(row.seat_limit, None, "no plan means no ceiling");
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_by_slug_returns_none_for_an_unknown_slug() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let found = crud::find_organization_by_slug(&db.pool, &unique("nope"))
        .await
        .expect("lookup succeeds");

    assert!(found.is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_by_slug_carries_the_plans_seat_limit() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let plan_id = unique("plan");
    insert_plan(&db.pool, &plan_id, Some(25), None, 0).await;
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.plan_id = Some(&plan_id);
    insert_org(&db.pool, &spec).await;

    let found = crud::find_organization_by_slug(&db.pool, &org_id)
        .await
        .expect("lookup succeeds")
        .expect("the organization exists");

    assert_eq!(found.seat_limit, Some(25));
    assert_eq!(found.plan_id.as_deref(), Some(plan_id.as_str()));
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_for_user_returns_none_for_an_unattached_user() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("floating")).await;

    let org = crud::find_organization_for_user(&db.pool, &user)
        .await
        .expect("lookup succeeds");

    assert!(org.is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_for_user_answers_with_the_slug_even_when_suspended() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.status = "suspended";
    insert_org(&db.pool, &spec).await;
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("suspended")).await;
    insert_member(&db.pool, &user, &org_id, "member").await;

    let org = crud::find_organization_for_user(&db.pool, &user)
        .await
        .expect("lookup succeeds");

    assert_eq!(
        org.as_deref(),
        Some(org_id.as_str()),
        "'who owns this user' stays true while a customer is suspended"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn get_platform_membership_is_false_for_a_customer_admin() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let org_id = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&org_id, &org_id)).await;
    let user = insert_user_full(
        &db.pool,
        &unique("user"),
        &unclaimed_email("custadmin"),
        None,
        &["user".to_owned(), "admin".to_owned()],
        "active",
    )
    .await;
    insert_member(&db.pool, &user, &org_id, "owner").await;

    let is_platform = crud::get_platform_membership(&db.pool, &user)
        .await
        .expect("lookup succeeds");

    assert!(
        !is_platform,
        "the admin role alone must not open the cross-customer console"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn get_platform_membership_is_true_inside_the_platform_tenant() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("operator")).await;
    insert_member(&db.pool, &user, HOUSE, "owner").await;

    let is_platform = crud::get_platform_membership(&db.pool, &user)
        .await
        .expect("lookup succeeds");

    assert!(is_platform);
    db.cleanup().await;
}

#[tokio::test]
async fn get_platform_membership_is_false_for_an_unknown_user() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let is_platform = crud::get_platform_membership(&db.pool, &UserId::new(unique("absent")))
        .await
        .expect("lookup succeeds");

    assert!(!is_platform);
    db.cleanup().await;
}

#[tokio::test]
async fn list_members_returns_only_this_organizations_members() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let mine = unique("org");
    let theirs = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&mine, &mine)).await;
    insert_org(&db.pool, &OrgSpec::active(&theirs, &theirs)).await;
    let a = insert_user(&db.pool, &unique("user"), &unclaimed_email("a")).await;
    let b = insert_user(&db.pool, &unique("user"), &unclaimed_email("b")).await;
    insert_member(&db.pool, &a, &mine, "owner").await;
    insert_member(&db.pool, &b, &theirs, "member").await;

    let members = crud::list_members(&db.pool, &mine)
        .await
        .expect("listing succeeds");

    assert_eq!(members.len(), 1);
    assert_eq!(members[0].user_id, a);
    assert_eq!(members[0].org_role, "owner");
    assert!(members[0].is_active);
    db.cleanup().await;
}

#[tokio::test]
async fn set_membership_moves_a_user_between_organizations() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let first = unique("org");
    let second = unique("org");
    insert_org(&db.pool, &OrgSpec::active(&first, &first)).await;
    insert_org(&db.pool, &OrgSpec::active(&second, &second)).await;
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("mover")).await;

    crud::set_membership(&db.pool, &user, &first, "member")
        .await
        .expect("first placement succeeds");
    crud::set_membership(&db.pool, &user, &second, "admin")
        .await
        .expect("the move succeeds");

    let members = crud::list_members(&db.pool, &second)
        .await
        .expect("listing succeeds");
    assert_eq!(
        members.len(),
        1,
        "one row per user is the tenancy invariant"
    );
    assert_eq!(members[0].org_role, "admin");
    assert!(
        crud::list_members(&db.pool, &first)
            .await
            .expect("listing succeeds")
            .is_empty()
    );
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_for_email_matches_the_domain_case_insensitively() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let domain = format!("{}.example", uuid::Uuid::new_v4().simple());
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.email_domains = vec![domain.clone()];
    insert_org(&db.pool, &spec).await;

    let found =
        crud::find_organization_for_email(&db.pool, &format!("Person@{}", domain.to_uppercase()))
            .await
            .expect("lookup succeeds");

    assert_eq!(found.as_deref(), Some(org_id.as_str()));
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_for_email_ignores_a_suspended_organization() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let domain = format!("{}.example", uuid::Uuid::new_v4().simple());
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.status = "suspended";
    spec.email_domains = vec![domain.clone()];
    insert_org(&db.pool, &spec).await;

    let found = crud::find_organization_for_email(&db.pool, &format!("person@{domain}"))
        .await
        .expect("lookup succeeds");

    assert!(found.is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn find_organization_for_email_returns_none_for_a_malformed_address() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    for candidate in ["no-at-sign", "trailing@"] {
        let found = crud::find_organization_for_email(&db.pool, candidate)
            .await
            .expect("lookup succeeds");
        assert!(found.is_none(), "{candidate} claims no organization");
    }
    db.cleanup().await;
}
