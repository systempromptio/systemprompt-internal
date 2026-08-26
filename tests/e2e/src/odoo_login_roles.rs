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

#[tokio::test]
async fn odoo_groups_drive_platform_roles_across_the_whole_login_lifecycle() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    stack
        .odoo
        .set_groups(Groups::XmlIds(vec![("base", "group_system")]));
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

    stack
        .odoo
        .set_groups(Groups::XmlIds(vec![("sales_team", "group_sale_salesman")]));
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
