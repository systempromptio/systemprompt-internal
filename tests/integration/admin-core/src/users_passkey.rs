//! `repositories::users::{odoo_identity, passkey}` — the Odoo credential side
//! table and passkey self-registration, which mirrors the provisioning rules
//! the federated door uses minus the federated mapping.

use systemprompt::identifiers::UserId;
use systemprompt_web_admin::repositories::users::{odoo_identity, passkey};

use crate::fixtures::{
    OrgSpec, insert_member, insert_org, insert_plan, insert_user, unclaimed_email, unique,
};
use crate::tempdb::TempDb;

// A fixed key for the sealed-credential round trip. Passed explicitly rather
// than exported into the environment: these tests run in one process with
// other suites, and mutating the environment underneath them is not worth a
// saved parameter.
const TEST_KEY: [u8; 32] = [7u8; 32];

#[tokio::test]
async fn odoo_identity_find_returns_none_before_any_link() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("odoo")).await;

    let found = odoo_identity::find(&db.pool, &user)
        .await
        .expect("lookup succeeds");

    assert!(
        found.is_none(),
        "an account that never linked Odoo has no identity row"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn odoo_identity_delete_is_fine_when_there_is_nothing_to_delete() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("unlink")).await;

    odoo_identity::delete(&db.pool, &user)
        .await
        .expect("deleting an absent link is not an error");

    assert!(
        odoo_identity::find(&db.pool, &user)
            .await
            .expect("lookup succeeds")
            .is_none()
    );
    db.cleanup().await;
}

#[tokio::test]
async fn odoo_identity_list_logins_is_sorted_and_deduplicated() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let a = insert_user(&db.pool, &unique("user"), &unclaimed_email("a")).await;
    let b = insert_user(&db.pool, &unique("user"), &unclaimed_email("b")).await;
    insert_link(&db.pool, &a, "zed@odoo.test", 42).await;
    insert_link(&db.pool, &b, "abe@odoo.test", 43).await;

    let logins = odoo_identity::list_odoo_logins(&db.pool)
        .await
        .expect("listing succeeds");

    assert_eq!(logins, vec!["abe@odoo.test", "zed@odoo.test"]);
    db.cleanup().await;
}

#[tokio::test]
async fn odoo_identity_relink_overwrites_login_uid_and_key() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("relink")).await;

    insert_link(&db.pool, &user, "first@odoo.test", 11).await;
    insert_link(&db.pool, &user, "second@odoo.test", 22).await;

    let found = odoo_identity::find(&db.pool, &user)
        .await
        .expect("lookup succeeds")
        .expect("the link exists");
    assert_eq!(found.odoo_login, "second@odoo.test");
    assert_eq!(
        found.odoo_uid, 22,
        "re-linking replaces the cached uid, which execute_kw calls are made with"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn odoo_identity_stores_the_api_key_sealed_not_in_the_clear() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("sealed")).await;
    let api_key = "odoo-api-key-plaintext";

    let sealed = odoo_identity::seal_with(&TEST_KEY, api_key).expect("sealing succeeds");
    sqlx::query(
        "INSERT INTO odoo_identity (user_id, odoo_login, odoo_uid, odoo_api_key_encrypted) \
         VALUES ($1, $2, $3, $4)",
    )
    .bind(user.as_str())
    .bind("sealed@odoo.test")
    .bind(99_i32)
    .bind(&sealed)
    .execute(&*db.pool)
    .await
    .expect("insert succeeds");

    let stored: String =
        sqlx::query_scalar("SELECT odoo_api_key_encrypted FROM odoo_identity WHERE user_id = $1")
            .bind(user.as_str())
            .fetch_one(&*db.pool)
            .await
            .expect("the row exists");

    assert!(
        !stored.contains(api_key),
        "the plaintext API key must never reach a database column"
    );
    assert_eq!(
        odoo_identity::open_with(&TEST_KEY, &stored).expect("opening succeeds"),
        api_key,
        "and the sealed value must round-trip under the same key"
    );
    db.cleanup().await;
}

// Insert a link row directly, sealing under [`TEST_KEY`]. The repository's own
// `insert` reaches for the deployment master key, which this process has no
// business setting.
async fn insert_link(pool: &sqlx::PgPool, user: &UserId, login: &str, uid: i32) {
    let sealed = odoo_identity::seal_with(&TEST_KEY, "key").expect("sealing succeeds");
    sqlx::query(
        "INSERT INTO odoo_identity (user_id, odoo_login, odoo_uid, odoo_api_key_encrypted) \
         VALUES ($1, $2, $3, $4) \
         ON CONFLICT (user_id) DO UPDATE \
         SET odoo_login = EXCLUDED.odoo_login, odoo_uid = EXCLUDED.odoo_uid",
    )
    .bind(user.as_str())
    .bind(login)
    .bind(uid)
    .bind(&sealed)
    .execute(pool)
    .await
    .expect("insert succeeds");
}

#[tokio::test]
async fn passkey_find_user_by_email_returns_none_for_a_stranger() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let found = passkey::find_user_by_email(&db.pool, &unclaimed_email("stranger"))
        .await
        .expect("lookup succeeds");

    assert!(found.is_none());
    db.cleanup().await;
}

#[tokio::test]
async fn passkey_find_user_by_email_reports_no_credential_yet() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("passkey");
    let user = insert_user(&db.pool, &unique("user"), &email).await;

    let found = passkey::find_user_by_email(&db.pool, &email.to_uppercase())
        .await
        .expect("lookup succeeds")
        .expect("the account exists");

    assert_eq!(found.id, user);
    assert!(
        !found.has_passkey,
        "a fresh account has no webauthn credential"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn passkey_count_webauthn_credentials_starts_at_zero() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("count")).await;

    let count = passkey::count_webauthn_credentials(&db.pool, &user)
        .await
        .expect("count succeeds");

    assert_eq!(count, 0);
    db.cleanup().await;
}

#[tokio::test]
async fn passkey_count_webauthn_credentials_is_zero_for_an_unknown_user() {
    let Some(db) = TempDb::create().await else {
        return;
    };

    let count = passkey::count_webauthn_credentials(&db.pool, &UserId::new(unique("absent")))
        .await
        .expect("count succeeds");

    assert_eq!(
        count, 0,
        "get_-shaped counts report zero rather than erroring on an unknown subject"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn passkey_insert_setup_token_stores_a_credential_link_token() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = insert_user(&db.pool, &unique("user"), &unclaimed_email("token")).await;
    let expires = chrono::Utc::now() + chrono::Duration::minutes(15);

    passkey::insert_setup_token(&db.pool, &user, "hash-of-token", expires)
        .await
        .expect("insert succeeds");

    let purpose: String =
        sqlx::query_scalar("SELECT purpose FROM webauthn_setup_tokens WHERE user_id = $1")
            .bind(user.as_str())
            .fetch_one(&*db.pool)
            .await
            .expect("the token row exists");
    assert_eq!(purpose, "credential_link");
    db.cleanup().await;
}

#[tokio::test]
async fn passkey_insert_passkey_user_provisions_like_the_sso_door() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let email = unclaimed_email("selfreg");

    let user_id = passkey::insert_passkey_user(&db.pool, &email, "Self Registered")
        .await
        .expect("self-registration succeeds on an unclaimed domain");

    let (name, display, verified): (String, Option<String>, bool) =
        sqlx::query_as("SELECT name, display_name, email_verified FROM users WHERE id = $1")
            .bind(user_id.as_str())
            .fetch_one(&*db.pool)
            .await
            .expect("the new user exists");
    assert_eq!(
        name, email,
        "name carries the email to dodge the uniqueness constraint"
    );
    assert_eq!(display.as_deref(), Some("Self Registered"));
    assert!(
        verified,
        "the domain allowlist is the gate, so the address is trusted"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn passkey_insert_passkey_user_refuses_a_full_plan() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let domain = format!("{}.example", uuid::Uuid::new_v4().simple());
    let plan_id = unique("plan");
    insert_plan(&db.pool, &plan_id, Some(1), None, 0).await;
    let org_id = unique("org");
    let mut spec = OrgSpec::active(&org_id, &org_id);
    spec.plan_id = Some(&plan_id);
    spec.email_domains = vec![domain.clone()];
    insert_org(&db.pool, &spec).await;
    let sitting = insert_user(&db.pool, &unique("user"), &format!("first@{domain}")).await;
    insert_member(&db.pool, &sitting, &org_id, "member").await;

    let refused = passkey::insert_passkey_user(&db.pool, &format!("second@{domain}"), "Second")
        .await
        .expect_err("the seat limit is enforced on this door too");

    assert!(refused.to_string().contains("seat limit reached"));
    db.cleanup().await;
}
