//! The Odoo account-linking endpoints, driven end-to-end.
//!
//! These three routes are the only way a per-user Odoo credential enters the
//! system, so the properties worth pinning are the refusals: an anonymous
//! caller must not reach them at all, a malformed link request must not store
//! anything, and a deployment with no Odoo connection configured must say so
//! rather than accept a credential it can never have validated.
//!
//! What is deliberately not driven here is a successful link. That requires a
//! live Odoo to answer `common.authenticate`, and a contract suite that stood
//! up an Odoo would be testing Odoo. The validation call itself is asserted in
//! the unit suite against a parsed envelope.

use axum::http::StatusCode;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal};

const STATUS_PATH: &str = "/admin/api/profile/odoo";
const LINK_PATH: &str = "/admin/api/profile/odoo/link";
const UNLINK_PATH: &str = "/admin/api/profile/odoo/unlink";

async fn app(db: &TempDb) -> App {
    let credentials = principal::provision(&db.pool).await;
    App::new(&db.pool, credentials)
}

#[tokio::test]
async fn an_anonymous_caller_is_bounced_from_every_odoo_route() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    for call in [
        Call::get(STATUS_PATH, Principal::Anonymous),
        Call::json("post", LINK_PATH, Principal::Anonymous, "{}"),
        Call::json("post", UNLINK_PATH, Principal::Anonymous, "{}"),
    ] {
        let path = call.path;
        let (status, _) = app.call(call).await;
        assert!(
            status.is_redirection() || status == StatusCode::UNAUTHORIZED,
            "{path} let an anonymous caller through with {status}"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn an_ordinary_user_can_read_their_own_link_status() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    let (status, body) = app.call(Call::get(STATUS_PATH, Principal::NonAdmin)).await;

    assert_eq!(
        status,
        StatusCode::OK,
        "linking Odoo is a profile action, not an admin one: {body}"
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&body).unwrap_or_else(|e| panic!("JSON body: {e}\n{body}"));
    assert_eq!(
        parsed["linked"], false,
        "a user who has never linked reports unlinked, not an error"
    );
    assert!(
        parsed.get("odoo_login").is_none(),
        "there is no login to report yet: {body}"
    );
}

#[tokio::test]
async fn linking_without_a_login_or_key_is_refused_before_any_network_call() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    for body in [
        r#"{"login":"","api_key":"secret"}"#,
        r#"{"login":"jo@example.com","api_key":"   "}"#,
    ] {
        let (status, response) = app
            .call(Call::json("post", LINK_PATH, Principal::NonAdmin, body))
            .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "an empty credential half must be rejected here, not sent to Odoo: {response}"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn linking_reports_an_unconfigured_deployment_rather_than_storing_anything() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    let (status, body) = app
        .call(Call::json(
            "post",
            LINK_PATH,
            Principal::NonAdmin,
            r#"{"login":"jo@example.com","api_key":"an-api-key"}"#,
        ))
        .await;

    // With no ODOO_URL / ODOO_DB — a fresh install — the link is refused as
    // unavailable. A dev shell may have both set (a live local Odoo); the
    // credentials above are bogus there, so Odoo itself refuses them. Either
    // way the invariant below holds: nothing is stored.
    let configured =
        std::env::var("ODOO_URL").is_ok_and(|v| !v.is_empty()) && std::env::var("ODOO_DB").is_ok();
    if configured {
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "a configured server must refuse bogus credentials via Odoo: {body}"
        );
    } else {
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "an unconfigured server is unavailable, not a bad request: {body}"
        );
    }

    let (_, status_body) = app.call(Call::get(STATUS_PATH, Principal::NonAdmin)).await;
    let parsed: serde_json::Value =
        serde_json::from_str(&status_body).unwrap_or_else(|e| panic!("JSON: {e}"));
    assert_eq!(
        parsed["linked"], false,
        "a failed link must leave no credential behind: {status_body}"
    );
    assert_eq!(
        parsed["configured"],
        serde_json::Value::Bool(configured),
        "and the page needs to know whether the form can work"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn unlinking_when_nothing_is_linked_succeeds() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    let (status, body) = app
        .call(Call::json("post", UNLINK_PATH, Principal::NonAdmin, "{}"))
        .await;

    assert_eq!(
        status,
        StatusCode::OK,
        "disconnect is idempotent — the requested state is already true: {body}"
    );
    db.cleanup().await;
}

// The sign-in endpoint. Unlike linking, this one is reachable by an anonymous
// caller by design — it is the front door — so what has to hold is that it
// refuses everything it cannot prove, and that it refuses locally, before any
// Odoo round trip, whenever the request is malformed.

const LOGIN_PATH: &str = "/admin/auth/odoo/login";

fn login_body(login: &str, credential: &str) -> String {
    format!(
        r#"{{"login":"{login}","credential":"{credential}",
            "client_id":"marketplace-admin",
            "redirect_uri":"http://localhost:8080/admin/login",
            "code_challenge":"E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM",
            "code_challenge_method":"S256"}}"#
    )
}

#[tokio::test]
async fn signing_in_without_both_halves_is_refused_before_any_network_call() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    for body in [
        login_body("", "secret"),
        login_body("jo@example.com", "   "),
    ] {
        let (status, response) = app
            .call(Call::json("post", LOGIN_PATH, Principal::Anonymous, &body))
            .await;
        assert_eq!(
            status,
            StatusCode::BAD_REQUEST,
            "an empty credential half must be rejected here, not sent to Odoo: {response}"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn a_login_that_is_not_an_email_is_refused() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    // Odoo happily authenticates a bare "admin", but resolution keys on email
    // and every consumer of users.email assumes a real address.
    let (status, body) = app
        .call(Call::json(
            "post",
            LOGIN_PATH,
            Principal::Anonymous,
            &login_body("admin", "an-api-key"),
        ))
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a non-email Odoo login has nothing to provision against: {body}"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn signing_in_without_a_pkce_challenge_is_refused() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    // Without PKCE the issued code would be redeemable by anyone who
    // intercepted it, so a request that omits the challenge must not proceed
    // to Odoo, let alone mint one.
    let (status, body) = app
        .call(Call::json(
            "post",
            LOGIN_PATH,
            Principal::Anonymous,
            r#"{"login":"jo@example.com","credential":"an-api-key",
                "client_id":"marketplace-admin",
                "redirect_uri":"http://localhost:8080/admin/login",
                "code_challenge":"","code_challenge_method":""}"#,
        ))
        .await;

    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "a code minted without a PKCE challenge is a bearer token: {body}"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn signing_in_never_provisions_a_user_from_an_unproven_credential() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let app = app(&db).await;

    let email = "never-provisioned@example.com";
    let (status, body) = app
        .call(Call::json(
            "post",
            LOGIN_PATH,
            Principal::Anonymous,
            &login_body(email, "not-a-real-key"),
        ))
        .await;

    // Unconfigured deployment: 503. Configured one (a dev shell with a live
    // Odoo): the bogus credential is refused, 401. Never a session.
    assert!(
        status == StatusCode::SERVICE_UNAVAILABLE || status == StatusCode::UNAUTHORIZED,
        "an unproven credential must not sign anyone in, got {status}: {body}"
    );

    let provisioned: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE LOWER(email) = $1")
        .bind(email)
        .fetch_one(&*db.pool)
        .await
        .unwrap_or_else(|e| panic!("count users: {e}"));
    assert_eq!(
        provisioned, 0,
        "auto-provisioning must happen only after Odoo confirms the credential"
    );
    db.cleanup().await;
}
