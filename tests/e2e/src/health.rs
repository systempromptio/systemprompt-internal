//! The skeleton smoke: the full router boots against this checkout's real
//! `services/` tree and answers on the surfaces the other flows depend on.

use axum::http::StatusCode;

use crate::harness::stack::Stack;

#[tokio::test]
async fn the_full_router_boots_and_answers_health() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let (status, body) = stack.send("GET", "/health", None, None).await;
    assert_eq!(status, StatusCode::OK, "health answered: {body}");

    stack.db.cleanup().await;
}

#[tokio::test]
async fn the_bridge_manifest_requires_a_credential() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    let (anonymous, _) = stack.send("GET", "/v1/bridge/manifest", None, None).await;
    assert_eq!(
        anonymous,
        StatusCode::UNAUTHORIZED,
        "an anonymous manifest fetch must be refused, not served or 500ed"
    );

    let (garbage, body) = stack
        .send("GET", "/v1/bridge/manifest", Some("not-a-jwt"), None)
        .await;
    assert_eq!(
        garbage,
        StatusCode::UNAUTHORIZED,
        "a garbage bearer is a 401, never a 500: {body}"
    );

    let (authed, body) = stack
        .send("GET", "/v1/bridge/manifest", Some(&stack.admin_token), None)
        .await;
    assert_eq!(authed, StatusCode::OK, "an admin session fetches: {body}");

    stack.db.cleanup().await;
}
