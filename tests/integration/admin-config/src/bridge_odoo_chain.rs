//! The bridge → user → Odoo identity chain.
//!
//! A bridge credential authenticates as a systemprompt user
//! (`find_bridge_user`), and the odoo MCP server executes every tool call with
//! that user's linked Odoo credential (`odoo_identity`). These tests pin the
//! join between the two: the user behind a bridge credential resolves to their
//! Odoo link state, and an unlinked user resolves to `None` — the state the
//! Bridge Setup page nudges on and the odoo MCP server refuses with an
//! actionable message.

use systemprompt_web_admin::repositories::bridge::find_bridge_user;
use systemprompt_web_admin::repositories::users::odoo_identity;

use crate::fixtures::{insert_user, unique, user_id};
use crate::tempdb::TempDb;

async fn link_odoo(pool: &sqlx::PgPool, user: &str, login: &str, uid: i32) {
    // Insert the row directly: `odoo_identity::insert` seals the API key under
    // the deployment master key, which a test database does not have. The chain
    // under test reads only login/uid, never the sealed credential.
    sqlx::query(
        "INSERT INTO odoo_identity (user_id, odoo_login, odoo_uid, odoo_api_key_encrypted)
         VALUES ($1, $2, $3, 'deadbeef')",
    )
    .bind(user)
    .bind(login)
    .bind(uid)
    .execute(pool)
    .await
    .expect("insert odoo identity");
}

#[tokio::test]
async fn a_bridge_user_resolves_to_their_linked_odoo_identity() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = unique("u");
    insert_user(&db.pool, &user).await;
    link_odoo(&db.pool, &user, "ed@example.test", 7).await;

    let bridge_user = find_bridge_user(&db.pool, &user_id(&user))
        .await
        .expect("look up bridge user")
        .expect("the user exists");
    let identity = odoo_identity::find(&db.pool, &user_id(&bridge_user.id))
        .await
        .expect("look up odoo identity")
        .expect("the link exists");

    assert_eq!(identity.odoo_login, "ed@example.test");
    assert_eq!(identity.odoo_uid, 7);

    db.cleanup().await;
}

#[tokio::test]
async fn a_bridge_user_without_a_link_resolves_to_none() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = unique("u");
    insert_user(&db.pool, &user).await;

    let bridge_user = find_bridge_user(&db.pool, &user_id(&user))
        .await
        .expect("look up bridge user")
        .expect("the user exists");
    let identity = odoo_identity::find(&db.pool, &user_id(&bridge_user.id))
        .await
        .expect("look up odoo identity");

    assert!(
        identity.is_none(),
        "an unlinked user is None — the state the setup page nudges on"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn relinking_overwrites_rather_than_duplicates() {
    let Some(db) = TempDb::create().await else {
        return;
    };
    let user = unique("u");
    insert_user(&db.pool, &user).await;
    link_odoo(&db.pool, &user, "old@example.test", 7).await;

    sqlx::query(
        "INSERT INTO odoo_identity (user_id, odoo_login, odoo_uid, odoo_api_key_encrypted)
         VALUES ($1, 'new@example.test', 8, 'deadbeef')
         ON CONFLICT (user_id) DO UPDATE
         SET odoo_login = EXCLUDED.odoo_login, odoo_uid = EXCLUDED.odoo_uid",
    )
    .bind(&user)
    .execute(&*db.pool)
    .await
    .expect("relink");

    let identity = odoo_identity::find(&db.pool, &user_id(&user))
        .await
        .expect("look up odoo identity")
        .expect("the link exists");
    assert_eq!(identity.odoo_login, "new@example.test");
    assert_eq!(identity.odoo_uid, 8);

    db.cleanup().await;
}
