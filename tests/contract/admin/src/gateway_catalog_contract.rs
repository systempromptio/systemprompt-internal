//! The per-user gateway catalog and the after-the-fact ACL detector.
//!
//! Both surfaces are redundant with core's `AuthzDecisionHook`, so nothing
//! downstream fails when they break. The resolver defaults to deny, and the
//! detector sweeps every recent request in the database, so assertions
//! target rows written for the case's own user, never a global count.

use axum::http::StatusCode;
use serde_json::Value;
use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal, seed};

// `contract-claude` matches the model `seed::insert_request` writes.
const CLAUDE_ROUTE: &str = "contract-claude";
const GPT_ROUTE: &str = "contract-gpt";

const DETECT: &str = "/api/public/admin/gateway/acl/detect";

fn parse(body: &str) -> Value {
    serde_json::from_str(body).unwrap_or_else(|e| panic!("response is JSON: {e}\n{body}"))
}

fn catalog_path(user_id: &str) -> String {
    format!("/api/public/admin/gateway/catalog/for-user/{user_id}")
}

async fn catalog_for(app: &App, user_id: &str) -> Vec<String> {
    let (status, body) = app
        .call(Call::get(&catalog_path(user_id), Principal::Admin))
        .await;
    assert_eq!(status, StatusCode::OK, "catalog: {body}");
    let parsed = parse(&body);
    assert_eq!(parsed["user_id"], user_id);
    let mut ids: Vec<String> = parsed["routes"]
        .as_array()
        .expect("routes is an array")
        .iter()
        .map(|r| r["id"].as_str().unwrap_or_default().to_owned())
        .collect();
    ids.sort();
    ids
}

async fn rule_for_user(pool: &PgPool, route_id: &str, user_id: &UserId, access: &str) {
    seed::insert_acl_rule(
        pool,
        "gateway_route",
        route_id,
        "user",
        user_id.as_str(),
        access,
    )
    .await;
}

async fn sweep_and_count(app: &App, pool: &PgPool, user_id: &UserId) -> i64 {
    let (status, body) = app.call(Call::get(DETECT, Principal::Admin)).await;
    assert_eq!(status, StatusCode::OK, "sweep: {body}");
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM governance_decisions
         WHERE user_id = $1 AND policy = 'gateway_acl'",
    )
    .bind(user_id.as_str())
    .fetch_one(pool)
    .await
    .expect("count detector decisions")
}

#[tokio::test]
async fn the_catalog_returns_only_the_routes_a_user_is_granted() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let id = seed::unique("catalog-user");
    let user = seed::insert_user(&db.pool, &id, &format!("{id}@contract.test")).await;

    assert!(
        catalog_for(&app, &id).await.is_empty(),
        "an unconfigured user sees no routes"
    );

    rule_for_user(&db.pool, CLAUDE_ROUTE, &user, "allow").await;
    assert_eq!(
        catalog_for(&app, &id).await,
        vec![CLAUDE_ROUTE.to_owned()],
        "only the granted route appears"
    );

    rule_for_user(&db.pool, GPT_ROUTE, &user, "allow").await;
    assert_eq!(
        catalog_for(&app, &id).await,
        vec![CLAUDE_ROUTE.to_owned(), GPT_ROUTE.to_owned()],
        "both grants are honoured"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn a_grant_to_one_user_does_not_reach_another() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let granted_id = seed::unique("catalog-granted");
    let granted = seed::insert_user(
        &db.pool,
        &granted_id,
        &format!("{granted_id}@contract.test"),
    )
    .await;
    let other_id = seed::unique("catalog-other");
    seed::insert_user(&db.pool, &other_id, &format!("{other_id}@contract.test")).await;

    rule_for_user(&db.pool, CLAUDE_ROUTE, &granted, "allow").await;

    assert_eq!(catalog_for(&app, &granted_id).await, vec![CLAUDE_ROUTE]);
    assert!(
        catalog_for(&app, &other_id).await.is_empty(),
        "the rule is scoped to the user it names"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn the_catalog_and_the_detector_are_both_behind_the_admin_gate() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    // The admin middleware runs first, so the handler's "or the subject
    // themselves" carve-out is unreachable over HTTP.
    for path in [
        catalog_path(&seed::unique("someone-else")),
        DETECT.to_owned(),
    ] {
        let (status, body) = app.call(Call::get(&path, Principal::NonAdmin)).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "non-admin {path}: {body}");
        assert_eq!(
            parse(&body)["error"],
            "Role required: platform_admin, admin, project_manager",
            "the console tier names the roles it accepts"
        );
    }

    let (status, body) = app
        .call(Call::get(
            &catalog_path(&seed::unique("ghost")),
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown user: {body}");
    assert_eq!(parse(&body)["error"], "User not found");

    db.cleanup().await;
}

#[tokio::test]
async fn the_detector_echoes_the_window_it_swept() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let (status, body) = app.call(Call::get(DETECT, Principal::Admin)).await;
    assert_eq!(status, StatusCode::OK, "default window: {body}");
    assert_eq!(parse(&body)["since_minutes"], 60);

    let (status, body) = app
        .call(Call::get(
            &format!("{DETECT}?since_minutes=5"),
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "explicit window: {body}");
    assert_eq!(parse(&body)["since_minutes"], 5);

    db.cleanup().await;
}

#[tokio::test]
async fn the_detector_records_a_request_that_should_have_been_denied() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let id = seed::unique("detect-user");
    let user = seed::insert_user(&db.pool, &id, &format!("{id}@contract.test")).await;
    let session = seed::unique("detect-session");
    seed::insert_session(&db.pool, &session, &user).await;

    // Grant first, or the case cannot tell a working detector from one that
    // flags everything.
    rule_for_user(&db.pool, CLAUDE_ROUTE, &user, "allow").await;

    let request_id = seed::unique("detect-request");
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: request_id.clone(),
            user_id: &user,
            session_id: Some(&session),
            trace_id: None,
            context_id: None,
            status: "completed",
        },
    )
    .await;

    assert_eq!(
        sweep_and_count(&app, &db.pool, &user).await,
        0,
        "a request the ACL permits is not flagged"
    );

    sqlx::query("DELETE FROM access_control_rules WHERE rule_value = $1")
        .bind(user.as_str())
        .execute(db.pool.as_ref())
        .await
        .expect("revoke the grant");

    assert_eq!(
        sweep_and_count(&app, &db.pool, &user).await,
        1,
        "the now-denied request is flagged"
    );

    let (decision, policy, actor_id, reason, evaluated) =
        sqlx::query_as::<_, (String, String, String, String, Option<Value>)>(
            "SELECT decision, policy, actor_id, reason, evaluated_rules
             FROM governance_decisions WHERE user_id = $1 AND policy = 'gateway_acl'",
        )
        .bind(user.as_str())
        .fetch_one(db.pool.as_ref())
        .await
        .expect("a decision row was written");

    // `decision` is CHECK-constrained to allow/deny; policy and actor mark the
    // row as a redundancy check.
    assert_eq!(decision, "deny");
    assert_eq!(policy, "gateway_acl");
    assert_eq!(actor_id, "gateway_acl_detector");
    assert!(!reason.is_empty(), "the row says why it was denied");

    let evaluated = evaluated.expect("evaluated_rules is populated");
    assert_eq!(
        evaluated["ai_request_id"], request_id,
        "the audit points back at the request it judged"
    );
    assert_eq!(evaluated["matched_route_id"], CLAUDE_ROUTE);
    assert_eq!(evaluated["model"], "claude-contract-model");

    db.cleanup().await;
}

#[tokio::test]
async fn the_detector_skips_requests_outside_the_window_and_off_the_catalog() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let id = seed::unique("detect-window");
    let user = seed::insert_user(&db.pool, &id, &format!("{id}@contract.test")).await;
    let session = seed::unique("detect-window-session");
    seed::insert_session(&db.pool, &session, &user).await;
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: seed::unique("old-request"),
            user_id: &user,
            session_id: Some(&session),
            trace_id: None,
            context_id: None,
            status: "completed",
        },
    )
    .await;

    // A sweep ignoring `since_minutes` would re-flag all history on every run.
    sqlx::query(
        "UPDATE ai_requests SET created_at = NOW() - INTERVAL '3 hours' WHERE user_id = $1",
    )
    .bind(user.as_str())
    .execute(db.pool.as_ref())
    .await
    .expect("age the request");

    let (status, body) = app
        .call(Call::get(
            &format!("{DETECT}?since_minutes=30"),
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::OK, "windowed sweep: {body}");
    assert_eq!(
        detector_rows(&db.pool, &user).await,
        0,
        "a request older than the window is not swept"
    );

    // An unrouted model has no ACL to violate: skipped, not denied.
    sqlx::query(
        "UPDATE ai_requests SET created_at = NOW(), model = 'llama-3-70b' WHERE user_id = $1",
    )
    .bind(user.as_str())
    .execute(db.pool.as_ref())
    .await
    .expect("retarget at an unrouted model");

    assert_eq!(
        sweep_and_count(&app, &db.pool, &user).await,
        0,
        "no route matched the model, so there was nothing to judge"
    );

    db.cleanup().await;
}

#[tokio::test]
async fn already_rejected_requests_are_not_swept_again() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let id = seed::unique("detect-rejected");
    let user = seed::insert_user(&db.pool, &id, &format!("{id}@contract.test")).await;
    let session = seed::unique("detect-rejected-session");
    seed::insert_session(&db.pool, &session, &user).await;

    // Already refused by live enforcement; re-flagging would double count.
    seed::insert_request(
        &db.pool,
        &seed::RequestSpec {
            id: seed::unique("rejected-request"),
            user_id: &user,
            session_id: Some(&session),
            trace_id: None,
            context_id: None,
            status: "rejected",
        },
    )
    .await;

    assert_eq!(
        sweep_and_count(&app, &db.pool, &user).await,
        0,
        "a request enforcement already rejected is skipped"
    );

    db.cleanup().await;
}

async fn detector_rows(pool: &PgPool, user_id: &UserId) -> i64 {
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM governance_decisions
         WHERE user_id = $1 AND policy = 'gateway_acl'",
    )
    .bind(user_id.as_str())
    .fetch_one(pool)
    .await
    .expect("count detector decisions")
}
