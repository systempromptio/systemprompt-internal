//! Populated fleet-device contracts for credentials and bridge activity.
//!
//! The account detail tab already covers one person's PAT. These cases cover
//! the fleet view's separate tables: real holders are grouped, secret hashes
//! remain absent, revoked rows can be audited without offering another revoke,
//! and bridge restarts are folded into their current machine.

use axum::http::StatusCode;
use sqlx::PgPool;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal, seed};

struct PatRow<'a> {
    user_id: &'a str,
    name: &'a str,
    prefix: &'a str,
    hash: &'a str,
    revoked: bool,
}

async fn insert_pat(pool: &PgPool, pat: PatRow<'_>) -> String {
    let PatRow {
        user_id,
        name,
        prefix,
        hash,
        revoked,
    } = pat;
    let id = seed::unique("fleet-pat");
    sqlx::query(
        "INSERT INTO user_api_keys
             (id, user_id, name, key_prefix, key_hash, last_used_at, expires_at, revoked_at)
         VALUES ($1, $2, $3, $4, $5, clock_timestamp(),
                 clock_timestamp() + interval '14 days',
                 CASE WHEN $6 THEN clock_timestamp() ELSE NULL END)",
    )
    .bind(&id)
    .bind(user_id)
    .bind(name)
    .bind(prefix)
    .bind(hash)
    .bind(revoked)
    .execute(pool)
    .await
    .expect("insert PAT");
    id
}

#[tokio::test(flavor = "multi_thread")]
async fn fleet_pat_view_masks_hashes_and_scopes_revocation_to_live_credentials() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let owner_name = seed::unique("pat-owner");
    let owner = seed::insert_user(
        &db.pool,
        &owner_name,
        &format!("{owner_name}@contract.test"),
    )
    .await;
    let active_id = insert_pat(
        &db.pool,
        PatRow {
            user_id: owner.as_str(),
            name: "active fleet token",
            prefix: "sp_live_masked",
            hash: "this-is-the-secret-hash-not-a-display-value",
            revoked: false,
        },
    )
    .await;
    let revoked_id = insert_pat(
        &db.pool,
        PatRow {
            user_id: owner.as_str(),
            name: "revoked fleet token",
            prefix: "sp_dead_masked",
            hash: "another-secret-hash-not-a-display-value",
            revoked: true,
        },
    )
    .await;

    let (status, body) = app
        .call(Call::get("/admin/devices?tab=pats", Principal::Admin))
        .await;
    assert_eq!(status, StatusCode::OK, "PAT fleet page: {body}");
    assert!(body.contains(&format!("{owner_name}@contract.test")));
    assert!(body.contains("active fleet token"));
    assert!(body.contains("revoked fleet token"));
    assert!(body.contains("sp_live_masked"));
    assert!(body.contains("sp_dead_masked"));
    assert!(
        body.contains(&format!("data-revoke-id=\"{active_id}\"")),
        "the active credential is the revoke target"
    );
    assert!(
        !body.contains(&revoked_id),
        "a revoked credential is auditable but has no second revoke control"
    );
    assert!(
        !body.contains("this-is-the-secret-hash-not-a-display-value")
            && !body.contains("another-secret-hash-not-a-display-value"),
        "the fleet page never renders stored credential hashes"
    );

    let (status, active_body) = app
        .call(Call::get(
            "/admin/devices?tab=pats&state=active",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "active PAT filter: {active_body}");
    assert!(active_body.contains("active fleet token"));
    assert!(!active_body.contains("revoked fleet token"));

    let (status, revoked_body) = app
        .call(Call::get(
            "/admin/devices?tab=pats&state=revoked",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "revoked PAT filter: {revoked_body}");
    assert!(revoked_body.contains("revoked fleet token"));
    assert!(!revoked_body.contains("active fleet token"));
    db.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn bridge_fleet_folds_restarts_by_machine_and_stale_filter_excludes_live_hosts() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);
    let owner_name = seed::unique("bridge-owner");
    let owner = seed::insert_user(
        &db.pool,
        &owner_name,
        &format!("{owner_name}@contract.test"),
    )
    .await;
    let stale_name = seed::unique("bridge-stale-owner");
    let stale_owner = seed::insert_user(
        &db.pool,
        &stale_name,
        &format!("{stale_name}@contract.test"),
    )
    .await;
    sqlx::query(
        "INSERT INTO bridge_sessions
             (session_id, user_id, bridge_version, os, hostname, started_at, last_heartbeat_at,
              forwarded_total, tokens_in_total, tokens_out_total)
         VALUES ($1, $2, '1.0.0', 'linux', 'build-host', clock_timestamp() - interval '2 days',
                 clock_timestamp() - interval '1 day', 3, 10, 20),
                ($3, $2, '1.1.0', 'linux', 'build-host', clock_timestamp() - interval '1 day',
                 clock_timestamp(), 7, 30, 40),
                ($4, $5, '0.9.0', 'macos', 'stale-host', clock_timestamp() - interval '20 days',
                 clock_timestamp() - interval '10 days', 2, 5, 5)",
    )
    .bind(seed::unique("bridge-first"))
    .bind(owner.as_str())
    .bind(seed::unique("bridge-restart"))
    .bind(seed::unique("bridge-stale"))
    .bind(stale_owner.as_str())
    .execute(&*db.pool)
    .await
    .expect("seed bridge restarts and a stale host");

    let (status, body) = app
        .call(Call::get("/admin/devices", Principal::Admin))
        .await;
    assert_eq!(status, StatusCode::OK, "bridge fleet page: {body}");
    assert!(body.contains("build-host") && body.contains("1.1.0"));
    assert!(
        body.contains("2 sessions"),
        "restarts are folded under one host"
    );
    assert!(body.contains("stale-host") && body.contains("Stale"));

    let (status, stale_body) = app
        .call(Call::get("/admin/devices?stale=1", Principal::Admin))
        .await;
    assert_eq!(status, StatusCode::OK, "stale bridge filter: {stale_body}");
    assert!(stale_body.contains("stale-host"));
    assert!(
        !stale_body.contains("build-host"),
        "the stale filter must not include a host with a fresh heartbeat"
    );
    db.cleanup().await;
}
