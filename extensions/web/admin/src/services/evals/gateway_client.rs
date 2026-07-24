//! Judge and replay calls, placed through our own gateway.
//!
//! The web extension has no in-process `AiService` — `ExtensionContext` hands
//! out config and a database and nothing else — so inference here goes out the
//! front door: `POST /v1/messages` on the loopback address, with the operator's
//! own JWT and the `x-session-id` the gateway binds it to. Exactly the path Pi
//! takes.
//!
//! That is not a workaround so much as the honest arrangement: an eval run is
//! scope-checked, secret-scanned, rate-limited and audited like any other
//! client's traffic, and the cost we report per run is the gateway's own
//! recorded number rather than an estimate. Each call carries a unique
//! `x-gateway-conversation-id` so its `ai_requests` row can be found again.
//!
//! No sampling parameters are sent. Pinning `temperature: 0` would be the
//! obvious way to keep judging repeatable, but several models reject any
//! explicit temperature outright (gpt-5-mini: "does not support 0.0"), and a
//! judge that cannot call half the registry is worse than one that varies a
//! little. Repeatability comes from the rubric instead.

use serde::Serialize;
use serde_json::Value;
use systemprompt::identifiers::headers::GATEWAY_CONVERSATION_ID;
use systemprompt::identifiers::{GatewayConversationId, SessionId};

// Why: the operator's own session, not a service identity — a run can never
// reach a model the operator could not have called themselves.
#[derive(Debug, Clone)]
pub(crate) struct GatewayCredential {
    pub base_url: String,
    pub token: String,
    pub session_id: SessionId,
}

#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<Message<'a>>,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

// Why: carries the conversation id so the call's recorded cost can be found.
#[derive(Debug, Clone)]
pub(crate) struct GatewayAnswer {
    pub text: String,
    pub conversation_id: GatewayConversationId,
}

// Why: the gateway accepts only `ctx_` + 16 hex, so the tag cannot carry a run
// id; the returned value is what ties a call back to its cost row.
pub(crate) fn new_conversation_id() -> GatewayConversationId {
    GatewayConversationId::from_prefix_hash(u64::from_be_bytes(
        uuid::Uuid::new_v4().as_bytes()[..8]
            .try_into()
            .unwrap_or([0u8; 8]),
    ))
}

#[derive(Debug)]
pub(crate) struct CallParams<'a> {
    pub credential: &'a GatewayCredential,
    pub model: &'a str,
    pub system: Option<&'a str>,
    pub user: &'a str,
    pub max_tokens: u32,
    pub conversation_id: &'a GatewayConversationId,
}

// Why: every failure mode — transport, non-2xx, unparseable body, empty
// content — collapses to `None`, so the caller counts a failed item rather
// than inventing a score.
pub(crate) async fn call_messages(params: CallParams<'_>) -> Option<GatewayAnswer> {
    let body = MessagesRequest {
        model: params.model,
        max_tokens: params.max_tokens,
        system: params.system,
        messages: vec![Message {
            role: "user",
            content: params.user,
        }],
    };

    let url = format!(
        "{}/v1/messages",
        params.credential.base_url.trim_end_matches('/')
    );
    let response = reqwest::Client::new()
        .post(&url)
        .bearer_auth(&params.credential.token)
        .header("x-session-id", params.credential.session_id.as_str())
        .header(GATEWAY_CONVERSATION_ID, params.conversation_id.as_str())
        .header("anthropic-version", "2023-06-01")
        .json(&body)
        .send()
        .await
        .inspect_err(|e| tracing::warn!(error = %e, url, "eval gateway call failed"))
        .ok()?;

    let status = response.status();
    let payload = response
        .text()
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "eval gateway response unreadable"))
        .ok()?;

    if !status.is_success() {
        tracing::warn!(
            %status,
            body = %payload.chars().take(400).collect::<String>(),
            "eval gateway call rejected"
        );
        return None;
    }

    // JSON: protocol boundary — the provider's own `/v1/messages` response body,
    // whose shape varies by upstream and is read through
    // `extract::assistant_answer`.
    let json: Value = serde_json::from_str(&payload)
        .inspect_err(|e| tracing::warn!(error = %e, "eval gateway response was not JSON"))
        .ok()?;

    let text = super::extract::assistant_answer(Some(&json))?;
    Some(GatewayAnswer {
        text,
        conversation_id: params.conversation_id.clone(),
    })
}

// Why: the gateway is a passthrough, so there is no structured-output
// enforcement to lean on and a fenced or prefaced reply must still parse.
pub(crate) fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;

    for (idx, ch) in text[start..].char_indices() {
        if in_string {
            match ch {
                _ if escaped => escaped = false,
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {},
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&text[start..=start + idx]);
                }
            },
            _ => {},
        }
    }
    None
}
