//! Odoo sign-in end to end: JIT provisioning, group→role mapping, refresh.
//!
//! Odoo is the identity provider AND the role authority: change a user's
//! groups in Odoo and their platform roles must follow at the next sign-in,
//! never sooner, never on a failed lookup. The wiremock Odoo lets each test
//! flip the group answer between logins and watch `users.roles` at each step
//! — the wiring the admin/salesperson demo stands on.

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::harness::odoo_mock::{GOOD_CREDENTIAL, Groups};
use crate::harness::stack::Stack;

const LOGIN: &str = "odoo-person@systemprompt.local";

async fn login(stack: &Stack, credential: &str) -> (StatusCode, String) {
    stack.odoo_login(LOGIN, credential).await
}

async fn roles_of(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT roles FROM users WHERE email = $1")
        .bind(LOGIN)
        .fetch_one(pool)
        .await
        .expect("the JIT-provisioned user exists")
}

// `admin` is a gated role: `strip_gated_grants` refuses to let a federated
// claim ADD it unless this is set, including on first provision — otherwise
// control of an Odoo account would mint a platform admin. Removals are never
// gated. This test is about the group→role mapping, so it opts in; the gate
// itself is pinned by `an_odoo_admin_is_not_granted_platform_admin_by_default`.
//
// # Safety
// nextest runs each test in its own process, so this mutates no other test's
// environment, and it is set before the router handles any request.
fn permit_federated_admin_grants() {
    unsafe {
        std::env::set_var("FEDERATED_ROLES_MAY_GRANT_ADMIN", "1");
    }
}

#[tokio::test]
async fn odoo_groups_drive_platform_roles_across_the_whole_login_lifecycle() {
    permit_federated_admin_grants();
    let Some(stack) = Stack::create().await else {
        return;
    };

    // A real Odoo administrator holds base.group_user too — every internal
    // user does. Claiming group_system alone also made the RPC-viability probe
    // answer false, which is not a state Odoo can actually be in.
    stack.odoo.set_groups(Groups::XmlIds(vec![
        ("base", "group_user"),
        ("base", "group_system"),
    ]));
    let (status, body) = login(&stack, GOOD_CREDENTIAL).await;
    assert_eq!(status, StatusCode::OK, "first sign-in: {body}");
    assert!(
        body.contains("authorization_code"),
        "a sign-in mints an OAuth code: {body}"
    );
    assert_eq!(
        roles_of(&stack.db.pool).await,
        vec!["admin".to_owned(), "user".to_owned()],
        "an Odoo administrator arrives as a platform admin"
    );

    stack.odoo.set_groups(Groups::XmlIds(vec![
        ("base", "group_user"),
        ("sales_team", "group_sale_salesman"),
    ]));
    let (status, body) = login(&stack, GOOD_CREDENTIAL).await;
    assert_eq!(status, StatusCode::OK, "second sign-in: {body}");
    assert_eq!(
        roles_of(&stack.db.pool).await,
        vec!["user".to_owned()],
        "dropping the admin group in Odoo demotes the platform account at the next sign-in"
    );

    stack.odoo.set_groups(Groups::LookupFails);
    let (status, body) = login(&stack, GOOD_CREDENTIAL).await;
    assert_eq!(status, StatusCode::OK, "sign-in with broken lookup: {body}");
    assert_eq!(
        roles_of(&stack.db.pool).await,
        vec!["user".to_owned()],
        "a failed group lookup keeps the stored roles — it never grants and never strips"
    );

    stack.db.cleanup().await;
}

#[tokio::test]
async fn a_rejected_odoo_credential_provisions_nothing() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let (status, _body) = login(&stack, "wrong-credential").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    let minted: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE email = $1")
        .bind(LOGIN)
        .fetch_one(&*stack.db.pool)
        .await
        .expect("count succeeds");
    assert_eq!(minted, 0, "a refused login must leave no account behind");

    stack.db.cleanup().await;
}

// The gate `strip_gated_grants` enforces, with the env flag left at its
// default: an Odoo administrator signing in for the first time is provisioned
// as a plain user. Odoo may demote a platform admin but may never mint one,
// so the platform's admin list can never be silently inherited from Odoo's.
#[tokio::test]
async fn an_odoo_admin_is_not_granted_platform_admin_by_default() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    stack.odoo.set_groups(Groups::XmlIds(vec![
        ("base", "group_user"),
        ("base", "group_system"),
    ]));
    let (status, body) = login(&stack, GOOD_CREDENTIAL).await;
    assert_eq!(status, StatusCode::OK, "sign-in: {body}");
    assert_eq!(
        roles_of(&stack.db.pool).await,
        vec!["user".to_owned()],
        "an Odoo group must not mint a platform admin without \
         FEDERATED_ROLES_MAY_GRANT_ADMIN"
    );

    stack.db.cleanup().await;
}
