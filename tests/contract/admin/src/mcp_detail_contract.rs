//! A declared MCP detail page must expose its effective security declaration,
//! while unknown IDs and non-console callers never receive that metadata.

use axum::http::StatusCode;

use crate::app::{App, Call};
use crate::principal::Principal;
use crate::tempdb::TempDb;
use crate::{globals, principal};

#[tokio::test(flavor = "multi_thread")]
async fn configured_mcp_detail_shows_effective_oauth_and_binary_metadata() {
    if !globals::init() {
        return;
    }
    let Some(db) = TempDb::create().await else {
        return;
    };
    let credentials = principal::provision(&db.pool).await;
    let app = App::new(&db.pool, credentials);

    let (status, body) = app
        .call(Call::get("/admin/mcp/systemprompt", Principal::Admin))
        .await;
    assert_eq!(status, StatusCode::OK, "configured MCP detail: {body}");
    assert!(
        body.contains("systemprompt-mcp-agent"),
        "binary fact: {body}"
    );
    assert!(
        body.contains("OAuth") && body.contains("required"),
        "OAuth fact: {body}"
    );
    assert!(
        body.contains("Audience") && body.contains("mcp"),
        "audience fact: {body}"
    );
    assert!(
        body.contains("Source") && body.contains("services/mcp/systemprompt.yaml"),
        "source fact: {body}"
    );

    let (status, body) = app
        .call(Call::get(
            "/admin/mcp/not-a-declared-server",
            Principal::Admin,
        ))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "unknown MCP detail: {body}");
    let (status, headers) = app
        .response_headers(Call::get("/admin/mcp/systemprompt", Principal::NonAdmin))
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    assert_eq!(headers.location.as_deref(), Some("/admin/profile"));
    db.cleanup().await;
}
