//! The MCP wire, end to end: real binary, real protocol, real identity.
//!
//! This is the flow Cowork's Recent Activity dashboard runs: a signed-in
//! Odoo user's chatter tools, executed as that user via their auto-linked
//! credential. The note_search "%" round-trip is the regression that
//! motivated this suite — a wildcard query must return the notes the same
//! user just posted, not an empty feed.

use axum::http::StatusCode;

use crate::harness::mcp;
use crate::harness::odoo_mock::GOOD_CREDENTIAL;
use crate::harness::stack::Stack;

const LOGIN: &str = "e2e-notes@systemprompt.local";

#[tokio::test]
async fn a_signed_in_user_posts_and_finds_notes_over_the_mcp_wire() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    // Sign in first: JIT-provisions the platform user AND auto-links the Odoo
    // credential the MCP tools execute with.
    let (status, body) = stack.odoo_login(LOGIN, GOOD_CREDENTIAL).await;
    assert_eq!(status, StatusCode::OK, "odoo sign-in: {body}");
    let bearer = stack.token_for_email(LOGIN).await;

    let Some(server) = mcp::spawn_odoo_mcp(&stack.odoo.url()).await else {
        stack.db.cleanup().await;
        return;
    };

    let added = mcp::call_tool(
        server.port,
        &bearer,
        "note_add",
        serde_json::json!({ "model": "crm.lead", "res_id": 1, "body": "E2E wildcard note" }),
    )
    .await
    .expect("note_add succeeds");
    assert!(
        added.contains("Note posted"),
        "note_add confirms the post: {added}"
    );

    let listed = mcp::call_tool(
        server.port,
        &bearer,
        "note_list",
        serde_json::json!({ "model": "crm.lead", "res_id": 1 }),
    )
    .await
    .expect("note_list succeeds");
    assert!(
        listed.contains("E2E wildcard note"),
        "the thread shows the note: {listed}"
    );

    let searched = mcp::call_tool(
        server.port,
        &bearer,
        "note_search",
        serde_json::json!({ "query": "%", "limit": 50 }),
    )
    .await
    .expect("note_search succeeds");
    assert!(
        searched.contains("E2E wildcard note"),
        "a wildcard search is match-all, not a literal percent hunt: {searched}"
    );

    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn the_gateway_proxies_the_odoo_mcp_route() {
    let Some(stack) = Stack::create().await else {
        return;
    };

    // No subprocess is running here; the property is that the gateway OWNS the
    // route (auth challenge or upstream failure), rather than 404ing — a 404
    // means the proxy mount for services/mcp/odoo.yaml is gone.
    let (status, body) = stack
        .send("POST", "/api/v1/mcp/odoo/mcp", None, None)
        .await;
    assert_ne!(
        status,
        StatusCode::NOT_FOUND,
        "the MCP proxy route must be mounted: {body}"
    );

    stack.db.cleanup().await;
}
