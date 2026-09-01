//! `email_send` end to end: draft, confirm, hold, approve, send.
//!
//! The real `systemprompt-mcp-email` binary, over genuine HTTP, against the
//! throwaway database and a real SMTP server that records what it is given.
//! What is mocked is only the two things at the edges — the mail relay
//! ([`super::harness::smtp_mock`]) and Odoo ([`super::harness::odoo_mock`]).
//! Everything between them is production code: the MCP transport, OAuth,
//! RBAC, the governance chain, the approval store, the admin console route
//! that resolves a hold, and `lettre` building the message.
//!
//! The property all of this exists to defend is that **an email cannot leave
//! without a human**. Each test below is one way that could fail.

use rmcp::model::CallToolResponse;
use systemprompt_mcp_email::draft::{APPROVE_KEY, CONFIRM_FIELD};

use crate::harness::mcp;
use crate::harness::stack::Stack;

const TOOL: &str = "email_send";
const RECIPIENT: &str = "oliver@example.com";
const SUBJECT: &str = "Rollout complete";
const BODY: &str = "Everything is fixed and testable end to end.";

fn draft() -> serde_json::Value {
    serde_json::json!({
        "to": [RECIPIENT],
        "subject": SUBJECT,
        "body": BODY,
    })
}

// The confirm round, asserted on the wire rather than on our own types.
fn assert_is_confirm_round(response: &CallToolResponse) {
    let CallToolResponse::InputRequired(result) = response else {
        panic!("expected an input_required result, got: {response:?}");
    };
    let requests = result
        .input_requests
        .as_ref()
        .expect("the confirm round carries inputRequests");
    let request = requests
        .get(APPROVE_KEY)
        .expect("the request is keyed by APPROVE_KEY");
    let rendered = serde_json::to_value(request).expect("the request serializes");
    assert_eq!(rendered["method"], "elicitation/create");
    assert_eq!(
        rendered["params"]["requestedSchema"]["properties"][CONFIRM_FIELD]["type"],
        "boolean"
    );
    // The human must be able to see what they are approving even with no
    // artifact rendering.
    let message = rendered["params"]["message"]
        .as_str()
        .expect("the elicitation carries a message");
    assert!(message.contains(RECIPIENT), "message: {message}");
    assert!(message.contains(SUBJECT), "message: {message}");
    assert!(message.contains(BODY), "message: {message}");
}

#[tokio::test]
async fn the_first_call_never_sends_and_asks_a_human_to_confirm() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let Some(server) = mcp::spawn_email_mcp().await else {
        stack.db.cleanup().await;
        return;
    };
    let client = mcp::MrtrClient::connect(server.port, &stack.admin_token)
        .await
        .expect("connect to the email MCP server");

    let response = client
        .call_once(mcp::call_params(TOOL, draft()))
        .await
        .expect("the first round answers");

    assert_is_confirm_round(&response);
    assert_eq!(
        stack.smtp.count(),
        0,
        "round one must not send: the relay saw a message before any human confirmed"
    );

    client.cancel().await;
    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn declining_the_confirmation_sends_nothing() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let Some(server) = mcp::spawn_email_mcp().await else {
        stack.db.cleanup().await;
        return;
    };
    let client = mcp::MrtrClient::connect(server.port, &stack.admin_token)
        .await
        .expect("connect to the email MCP server");

    let declined = client
        .call_once(mcp::with_confirmation(
            mcp::call_params(TOOL, draft()),
            APPROVE_KEY,
            false,
            false,
        ))
        .await
        .expect("the declined round answers");

    // A decline is an ordinary outcome, not a protocol error: reporting it as
    // one would make a model retry the send.
    let CallToolResponse::Complete(result) = &declined else {
        panic!("a decline completes the call, got: {declined:?}");
    };
    assert_ne!(result.is_error, Some(true), "a decline is not an error");
    assert_eq!(stack.smtp.count(), 0, "a declined draft must not be sent");

    // Accepting the elicitation but leaving `confirm` false is the subtler
    // half: the client answered, but the human did not say yes.
    let unconfirmed = client
        .call_once(mcp::with_confirmation(
            mcp::call_params(TOOL, draft()),
            APPROVE_KEY,
            true,
            false,
        ))
        .await
        .expect("the unconfirmed round answers");
    assert!(matches!(unconfirmed, CallToolResponse::Complete(_)));
    assert_eq!(
        stack.smtp.count(),
        0,
        "accept-without-confirm must not be read as consent"
    );

    client.cancel().await;
    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn a_confirmed_send_reaches_the_relay_exactly_once_with_the_message_id_we_minted() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let Some(server) = mcp::spawn_email_mcp().await else {
        stack.db.cleanup().await;
        return;
    };
    // As the admin: `exempt_scopes: [admin]` on require_approval means this
    // caller is never held, so this exercises the in-band layer alone.
    let client = mcp::MrtrClient::connect(server.port, &stack.admin_token)
        .await
        .expect("connect to the email MCP server");

    let confirmed = client
        .call_once(mcp::with_confirmation(
            mcp::call_params(TOOL, draft()),
            APPROVE_KEY,
            true,
            true,
        ))
        .await
        .expect("the confirmed round answers");

    let CallToolResponse::Complete(result) = &confirmed else {
        panic!("an admin's confirmed send completes without a hold, got: {confirmed:?}");
    };
    let text = result
        .content
        .iter()
        .filter_map(|block| block.as_text().map(|t| t.text.clone()))
        .collect::<Vec<_>>()
        .join("\n");
    assert_ne!(result.is_error, Some(true), "the send failed: {text}");

    let received = stack.smtp.received();
    assert_eq!(
        received.len(),
        1,
        "exactly one copy must reach the relay, got {}",
        received.len()
    );
    let mail = &received[0];

    // The envelope, not the To: header, decides who actually gets a copy.
    assert_eq!(mail.rcpt_to, vec![RECIPIENT.to_owned()]);
    assert_eq!(
        mail.mail_from, "hello@systemprompt.io",
        "the envelope sender is our own domain — a forged From: fails SPF at the relay"
    );

    let message_id = mail
        .header("Message-ID")
        .expect("the message carries a Message-ID");
    assert!(
        message_id.starts_with('<') && message_id.ends_with('>'),
        "Message-ID must be in RFC5322 angle-bracket form: {message_id}"
    );
    assert!(
        message_id.contains("@systemprompt.io"),
        "the id we mint is scoped to the sending domain: {message_id}"
    );
    // The id has to be reported back, because it is the join key to the Odoo
    // chatter row and to the outbox ledger.
    assert!(
        text.contains(&message_id),
        "the result must report the Message-ID it sent; result was: {text}"
    );

    assert_eq!(mail.header("Subject").as_deref(), Some(SUBJECT));
    assert!(mail.body().contains(BODY), "body was: {}", mail.body());

    // The ledger closes the loop: this draft had no Odoo anchor, so it should
    // be `sent` rather than `logged`, and it must name the same id.
    let (status, outbox_id): (String, String) =
        sqlx::query_as("SELECT status, message_id FROM email_outbox WHERE message_id = $1")
            .bind(&message_id)
            .fetch_one(&*stack.db.pool)
            .await
            .expect("the send left an outbox row");
    assert_eq!(status, "sent");
    assert_eq!(outbox_id, message_id);

    client.cancel().await;
    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn a_non_admin_send_is_held_for_a_second_human_and_only_flies_once_approved() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let Some(server) = mcp::spawn_email_mcp().await else {
        stack.db.cleanup().await;
        return;
    };
    // The whole point of the second layer: the drafter confirmed their own
    // text, and a DIFFERENT person still has to allow it out. An admin caller
    // would be exempt, so this must run as the plain user.
    let client = mcp::MrtrClient::connect(server.port, &stack.user_token)
        .await
        .expect("connect to the email MCP server");

    let confirmed =
        mcp::with_confirmation(mcp::call_params(TOOL, draft()), APPROVE_KEY, true, true);

    // The call blocks for up to `hold_seconds` waiting on a decision, so the
    // approval has to happen while it is in flight.
    let call = tokio::spawn({
        let params = confirmed.clone();
        async move { client.call_once(params).await.map(|r| (client, r)) }
    });

    let call_id = wait_for_pending_approval(&stack).await;
    assert_eq!(
        stack.smtp.count(),
        0,
        "nothing may reach the relay while the call is still held"
    );

    let (status, _) = stack
        .send(
            "POST",
            &format!("/admin/governance/approvals/{call_id}/approve"),
            Some(&stack.admin_token),
            None,
        )
        .await;
    assert!(
        status.is_success() || status.is_redirection(),
        "the admin console must be able to approve a held call, got {status}"
    );

    let (client, response) = tokio::time::timeout(std::time::Duration::from_secs(90), call)
        .await
        .expect("the held call returns once approved")
        .expect("the call task did not panic")
        .expect("the approved round answers");

    // Whether the approval lands inside the first hold round or after a retry
    // is a timing detail of the wait; both are correct MRTR. What matters is
    // that it is not still asking, and that the mail went.
    if let CallToolResponse::InputRequired(_) = &response {
        let retried = client
            .call_once(confirmed)
            .await
            .expect("the retry after approval answers");
        assert!(
            matches!(retried, CallToolResponse::Complete(_)),
            "after a human approved, the retry must complete: {retried:?}"
        );
    }

    assert_eq!(
        stack.smtp.count(),
        1,
        "an approved send delivers exactly one copy"
    );
    assert_eq!(stack.smtp.received()[0].rcpt_to, vec![RECIPIENT.to_owned()]);

    let approver: Option<String> =
        sqlx::query_scalar("SELECT approver_username FROM approval_requests WHERE call_id = $1")
            .bind(&call_id)
            .fetch_one(&*stack.db.pool)
            .await
            .expect("the approval row survives the decision");
    assert!(
        approver.is_some(),
        "the approver must be stamped on the record, not just implied by the send"
    );

    client.cancel().await;
    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn a_denied_send_never_reaches_the_relay() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    let Some(server) = mcp::spawn_email_mcp().await else {
        stack.db.cleanup().await;
        return;
    };
    let client = mcp::MrtrClient::connect(server.port, &stack.user_token)
        .await
        .expect("connect to the email MCP server");

    let confirmed =
        mcp::with_confirmation(mcp::call_params(TOOL, draft()), APPROVE_KEY, true, true);
    let call = tokio::spawn({
        let params = confirmed.clone();
        async move { client.call_once(params).await.map(|r| (client, r)) }
    });

    let call_id = wait_for_pending_approval(&stack).await;
    let (status, _) = stack
        .send(
            "POST",
            &format!("/admin/governance/approvals/{call_id}/deny"),
            Some(&stack.admin_token),
            None,
        )
        .await;
    assert!(
        status.is_success() || status.is_redirection(),
        "got {status}"
    );

    let (client, response) = tokio::time::timeout(std::time::Duration::from_secs(90), call)
        .await
        .expect("the held call returns once denied")
        .expect("the call task did not panic")
        .expect("the denied round answers");

    if let CallToolResponse::InputRequired(_) = &response {
        let retried = client
            .call_once(confirmed)
            .await
            .expect("the retry after denial answers");
        // A refusal comes back as an isError result rather than a JSON-RPC
        // error, so a strict bridge still renders it.
        if let CallToolResponse::Complete(result) = &retried {
            assert_eq!(result.is_error, Some(true), "a denial is an error result");
        }
    }

    assert_eq!(
        stack.smtp.count(),
        0,
        "a denied send must never reach the relay"
    );

    client.cancel().await;
    drop(server);
    stack.db.cleanup().await;
}

#[tokio::test]
async fn the_gateway_proxies_the_email_mcp_route() {
    let Some(stack) = Stack::create().await else {
        return;
    };
    // The cheap check that services/mcp/email.yaml is still mounted. Anything
    // but a 404 means the route exists; what it answers is the subprocess
    // tests' business.
    // Why: no bearer and no body, matching the odoo mount check — the property
    // is that the gateway OWNS the route (auth challenge or upstream failure)
    // rather than 404ing, and adding credentials only moves the failure mode.
    let (status, body) = stack
        .send("POST", "/api/v1/mcp/email/mcp", None, None)
        .await;
    assert_ne!(
        status,
        axum::http::StatusCode::NOT_FOUND,
        "the MCP proxy route for services/mcp/email.yaml must be mounted: {body}"
    );
    stack.db.cleanup().await;
}

// Polls for the held call's id. The gate writes the row before it blocks, so
// this converges as soon as the server has parked the call.
async fn wait_for_pending_approval(stack: &Stack) -> String {
    for _ in 0..300 {
        let found: Option<String> = sqlx::query_scalar(
            "SELECT call_id FROM approval_requests WHERE tool_name = $1 AND status = 'pending'",
        )
        .bind(TOOL)
        .fetch_optional(&*stack.db.pool)
        .await
        .expect("query the approval store");
        if let Some(call_id) = found {
            return call_id;
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    panic!(
        "no approval request was ever opened for {TOOL} — was email_send removed from require_approval.patterns?"
    )
}
