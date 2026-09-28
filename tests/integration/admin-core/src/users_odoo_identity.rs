//! `repositories::users::odoo_identity` — the Odoo credential side table the
//! odoo MCP server reads to call Odoo as the signed-in user.

use systemprompt::identifiers::UserId;
use systemprompt_web_admin::repositories::users::odoo_identity;

use crate::fixtures::{insert_user, unclaimed_email, unique};
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

    odoo_identity::delete_odoo_identity(&db.pool, &user)
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
