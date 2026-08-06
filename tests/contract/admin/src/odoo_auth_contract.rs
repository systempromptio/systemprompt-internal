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

    // The contract suite runs with no ODOO_URL / ODOO_DB, which is exactly the
    // state a fresh install is in.
    assert_eq!(
        status,
        StatusCode::SERVICE_UNAVAILABLE,
        "an unconfigured server is unavailable, not a bad request: {body}"
    );

    let (_, status_body) = app.call(Call::get(STATUS_PATH, Principal::NonAdmin)).await;
    let parsed: serde_json::Value =
        serde_json::from_str(&status_body).unwrap_or_else(|e| panic!("JSON: {e}"));
    assert_eq!(
        parsed["linked"], false,
        "a failed link must leave no credential behind: {status_body}"
    );
    assert_eq!(
        parsed["configured"], false,
        "and the page needs to know why the form cannot work"
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
