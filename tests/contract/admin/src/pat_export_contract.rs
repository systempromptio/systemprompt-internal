//! A personal access token on the export routes, driven end-to-end.
//!
//! Exporting their own transcripts and figures with a PAT is how users and
//! their CI read this instance without a browser session. The PAT resolves to
//! its owner and meets the gates a session does, on `GET /admin/export/…` only:
//! every other admin route, and any write, refuses it. A user's own
//! transcripts are `?source=history` ("My conversations"); the console sources
//! need a console seat, from a PAT exactly as from a browser.

use axum::http::StatusCode;
use systemprompt::identifiers::UserId;
use systemprompt_web_admin::repositories::bridge::api_keys::issue_api_key;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal};

async fn pat_for(db: &TempDb, user: &UserId) -> String {
    issue_api_key(&db.pool, user, "export-contract", None)
        .await
        .expect("issue PAT")
        .secret
}

async fn status(app: &App, method: &str, path: &str, pat: &str) -> StatusCode {
    let call = if method == "get" {
        Call::get(path, Principal::Anonymous)
    } else {
        Call::json(method, path, Principal::Anonymous, "{}")
    };
    app.call_with_bearer(call, pat).await.0
}

#[tokio::test]
async fn a_users_pat_exports_their_transcripts_and_figures() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let user = credentials.non_admin_user_id.clone();
    let app = App::new(&db.pool, credentials);
    let pat = pat_for(&db, &user).await;

    for path in [
        "/admin/export/transcripts?source=history",
        "/admin/export/transcripts/preview?source=history",
        "/admin/export/history",
        "/admin/export/history/preview",
    ] {
        let got = status(&app, "get", path, &pat).await;
        assert!(
            got.is_success(),
            "a user's PAT must export {path}, got {got}"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn a_pat_reaches_no_further_than_its_owners_session() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let user = credentials.non_admin_user_id.clone();
    let app = App::new(&db.pool, credentials);
    let pat = pat_for(&db, &user).await;

    for path in [
        "/admin/export/transcripts",
        "/admin/export/transcripts?source=sessions",
        "/admin/export/sessions",
        "/admin/export/requests",
    ] {
        let session = app.call(Call::get(path, Principal::NonAdmin)).await.0;
        let token = status(&app, "get", path, &pat).await;
        assert!(
            !session.is_success(),
            "a user's session reaches console export {path}"
        );
        assert_eq!(
            token, session,
            "a PAT must answer {path} exactly as its owner's session does"
        );
    }
    db.cleanup().await;
}

#[tokio::test]
async fn a_pat_cannot_read_a_transcript_outside_its_owners_view() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let user = credentials.non_admin_user_id.clone();
    let app = App::new(&db.pool, credentials);
    let pat = pat_for(&db, &user).await;

    let got = status(
        &app,
        "get",
        "/admin/export/transcripts/00000000-0000-0000-0000-000000000000",
        &pat,
    )
    .await;
    assert_eq!(
        got,
        StatusCode::NOT_FOUND,
        "an unseen transcript is a 404, as in the console"
    );
    db.cleanup().await;
}

#[tokio::test]
async fn a_pat_is_refused_everywhere_but_export_reads() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let admin = credentials.admin_user_id.clone();
    let app = App::new(&db.pool, credentials);
    let pat = pat_for(&db, &admin).await;

    for (method, path) in [
        ("get", "/admin/"),
        ("get", "/admin/users"),
        ("get", "/admin/export/transcripts/abc/extra"),
        ("get", "/admin/exports/history"),
        ("post", "/admin/export/transcripts"),
        ("post", "/admin/export/history"),
    ] {
        let got = status(&app, method, path, &pat).await;
        assert!(
            got == StatusCode::UNAUTHORIZED
                || got == StatusCode::FORBIDDEN
                || got == StatusCode::METHOD_NOT_ALLOWED
                || got == StatusCode::NOT_FOUND,
            "a PAT must not reach {method} {path}, got {got}"
        );
    }
    db.cleanup().await;
}
