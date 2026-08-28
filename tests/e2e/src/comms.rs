//! Team comms over the real MCP wire: two sessions of one user, and the
//! guarantee that separates them.
//!
//! The property under test is the reason the whole design exists. A message
//! addressed to one session must reach that session and no other — not even a
//! sibling session belonging to the same person, signed in with the same
//! token, reading through the same tool. Everything else here (addressing,
//! read marks, the directory) is scaffolding for that one assertion.
//!
//! The stack's seeded `admin` and `e2e-user` stand in for two teammates, so
//! nothing here depends on Odoo sign-in.

use crate::harness::mcp;
use crate::harness::stack::Stack;

const RECIPIENT_EMAIL: &str = "ed+notadmin@systemprompt.io";
const RECIPIENT: &str = "e2e-user";
const SESSION_A: &str = "e2e-sess-a";
const SESSION_B: &str = "e2e-sess-b";
const SENDER_SESSION: &str = "e2e-sess-sender";
const HANDLE_A: &str = "alpha";

// The registry is normally populated by the hook plane, which does not run in
// this suite. Insert the row the addressing path reads, so the test exercises
// resolution and delivery rather than ingest.
async fn register_live_session(stack: &Stack, session_id: &str, user_id: &str, handle: &str) {
    sqlx::query(
        r"INSERT INTO plugin_session_summaries
            (id, session_id, user_id, handle, workspace, started_at, last_event_at)
          VALUES ($1, $2, $3, $4, $4, NOW(), NOW())
          ON CONFLICT (session_id) DO UPDATE
            SET handle = EXCLUDED.handle, last_event_at = NOW(), ended_at = NULL",
    )
    .bind(format!("sess_{session_id}"))
    .bind(session_id)
    .bind(user_id)
    .bind(handle)
    .execute(stack.db.pool.as_ref())
    .await
    .expect("register a live session");
}

async fn recipient_id(stack: &Stack) -> String {
    sqlx::query_scalar::<_, String>("SELECT id FROM users WHERE email = $1")
        .bind(RECIPIENT_EMAIL)
        .fetch_one(stack.db.pool.as_ref())
        .await
        .expect("the seeded recipient exists")
}

#[tokio::test]
async fn a_message_addressed_to_one_session_never_reaches_its_sibling() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let user_id = recipient_id(&stack).await;
    register_live_session(&stack, SESSION_A, &user_id, HANDLE_A).await;
    register_live_session(&stack, SESSION_B, &user_id, "beta").await;

    let Some(server) = mcp::spawn_comms_mcp().await else {
        stack.db.cleanup().await;
        return;
    };

    let sent = mcp::call_tool_as_session(
        server.port,
        &stack.admin_token,
        SENDER_SESSION,
        "comms_send",
        serde_json::json!({ "to": format!("@{RECIPIENT}/{HANDLE_A}"), "body": "for alpha only" }),
    )
    .await
    .expect("comms_send to a live session succeeds");
    assert!(
        sent.contains("as session"),
        "a live session address delivers as session class, not inbox: {sent}"
    );

    // Session B is the same user, the same token, the same tool.
    let b_inbox = mcp::call_tool_as_session(
        server.port,
        &stack.user_token,
        SESSION_B,
        "comms_inbox",
        serde_json::json!({}),
    )
    .await
    .expect("comms_inbox succeeds for session B");
    assert!(
        !b_inbox.contains("for alpha only"),
        "a sibling session must not see a message addressed to session A: {b_inbox}"
    );

    let a_inbox = mcp::call_tool_as_session(
        server.port,
        &stack.user_token,
        SESSION_A,
        "comms_inbox",
        serde_json::json!({}),
    )
    .await
    .expect("comms_inbox succeeds for session A");
    assert!(
        a_inbox.contains("for alpha only"),
        "the addressed session must see it: {a_inbox}"
    );

    // The read mark is per session: A's second look is empty, and nothing
    // about B changed when A read.
    let a_again = mcp::call_tool_as_session(
        server.port,
        &stack.user_token,
        SESSION_A,
        "comms_inbox",
        serde_json::json!({}),
    )
    .await
    .expect("comms_inbox succeeds on the second read");
    assert!(
        !a_again.contains("for alpha only"),
        "reading advances this session's high-water mark: {a_again}"
    );

    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn a_message_to_a_person_reaches_their_session_without_interrupting() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let user_id = recipient_id(&stack).await;
    register_live_session(&stack, SESSION_A, &user_id, HANDLE_A).await;

    let Some(server) = mcp::spawn_comms_mcp().await else {
        stack.db.cleanup().await;
        return;
    };

    let sent = mcp::call_tool_as_session(
        server.port,
        &stack.admin_token,
        SENDER_SESSION,
        "comms_send",
        serde_json::json!({ "to": format!("@{RECIPIENT}"), "body": "a quiet note" }),
    )
    .await
    .expect("comms_send to a person succeeds");
    assert!(
        sent.contains("as inbox"),
        "addressing a person must never produce an interrupting class: {sent}"
    );

    let inbox = mcp::call_tool_as_session(
        server.port,
        &stack.user_token,
        SESSION_A,
        "comms_inbox",
        serde_json::json!({}),
    )
    .await
    .expect("comms_inbox succeeds");
    assert!(
        inbox.contains("a quiet note"),
        "a user-addressed message is readable from any of their sessions: {inbox}"
    );

    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn addressing_an_idle_session_falls_back_to_the_inbox() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let user_id = recipient_id(&stack).await;
    register_live_session(&stack, SESSION_A, &user_id, HANDLE_A).await;
    // Age it past the liveness window without ending it — the crashed-laptop
    // case, which `ended_at IS NULL` alone would report as live forever.
    sqlx::query(
        "UPDATE plugin_session_summaries SET last_event_at = NOW() - INTERVAL '2 hours'
         WHERE session_id = $1",
    )
    .bind(SESSION_A)
    .execute(stack.db.pool.as_ref())
    .await
    .expect("age the session");

    let Some(server) = mcp::spawn_comms_mcp().await else {
        stack.db.cleanup().await;
        return;
    };

    let sent = mcp::call_tool_as_session(
        server.port,
        &stack.admin_token,
        SENDER_SESSION,
        "comms_send",
        serde_json::json!({ "to": format!("@{RECIPIENT}/{HANDLE_A}"), "body": "nobody home" }),
    )
    .await
    .expect("comms_send to an idle session still succeeds");
    assert!(
        sent.contains("as inbox"),
        "an idle target degrades to inbox rather than failing: {sent}"
    );
    assert!(
        sent.contains("idle"),
        "and says so, so the sender knows it will not be seen immediately: {sent}"
    );

    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn the_session_directory_lists_addressable_handles() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let user_id = recipient_id(&stack).await;
    register_live_session(&stack, SESSION_A, &user_id, HANDLE_A).await;

    let Some(server) = mcp::spawn_comms_mcp().await else {
        stack.db.cleanup().await;
        return;
    };

    let listed = mcp::call_tool_as_session(
        server.port,
        &stack.admin_token,
        SENDER_SESSION,
        "comms_sessions",
        serde_json::json!({}),
    )
    .await
    .expect("comms_sessions succeeds");
    assert!(
        listed.contains(HANDLE_A),
        "the directory must name the handle a sender types: {listed}"
    );

    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn an_unaddressed_user_sees_nothing() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let user_id = recipient_id(&stack).await;
    register_live_session(&stack, SESSION_A, &user_id, HANDLE_A).await;

    let Some(server) = mcp::spawn_comms_mcp().await else {
        stack.db.cleanup().await;
        return;
    };

    mcp::call_tool_as_session(
        server.port,
        &stack.admin_token,
        SENDER_SESSION,
        "comms_send",
        serde_json::json!({ "to": format!("@{RECIPIENT}"), "body": "private to the recipient" }),
    )
    .await
    .expect("comms_send succeeds");

    // The sender is a different user reading their own inbox.
    let sender_inbox = mcp::call_tool_as_session(
        server.port,
        &stack.admin_token,
        SENDER_SESSION,
        "comms_inbox",
        serde_json::json!({}),
    )
    .await
    .expect("comms_inbox succeeds for the sender");
    assert!(
        !sender_inbox.contains("private to the recipient"),
        "a message addressed to someone else must not appear in another user's inbox: \
         {sender_inbox}"
    );

    drop(server);
    stack.db.cleanup().await;
}
