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

use serde::Serialize;
use serde_json::Value;
use systemprompt::identifiers::headers::GATEWAY_CONVERSATION_ID;

/// The credential an eval run acts under: the operator's own session, not a
/// service identity. A run can therefore never reach a model the operator
/// could not have called themselves.
#[derive(Debug, Clone)]
pub(crate) struct GatewayCredential {
    pub base_url: String,
    pub token: String,
    pub session_id: String,
}

#[derive(Debug, Serialize)]
struct MessagesRequest<'a> {
    model: &'a str,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<&'a str>,
    messages: Vec<Message<'a>>,
    temperature: f32,
}

#[derive(Debug, Serialize)]
struct Message<'a> {
    role: &'a str,
    content: &'a str,
}

/// A completed gateway call: the text, and the conversation id its cost is
/// recorded under.
#[derive(Debug, Clone)]
pub(crate) struct GatewayAnswer {
    pub text: String,
    pub conversation_id: String,
}

#[derive(Debug)]
pub(crate) struct CallParams<'a> {
    pub credential: &'a GatewayCredential,
    pub model: &'a str,
    pub system: Option<&'a str>,
    pub user: &'a str,
    pub max_tokens: u32,
    pub conversation_id: &'a str,
}

/// Place one `/v1/messages` call. Every failure mode — transport, non-2xx,
/// unparseable body, empty content — collapses to `None`; the caller counts it
/// as a failed item rather than inventing a score.
pub(crate) async fn call_messages(params: CallParams<'_>) -> Option<GatewayAnswer> {
    let body = MessagesRequest {
        model: params.model,
        max_tokens: params.max_tokens,
        system: params.system,
        messages: vec![Message {
            role: "user",
            content: params.user,
        }],
        // Why: judging is a measurement, so it must not wander between runs.
        temperature: 0.0,
    };

    let url = format!("{}/v1/messages", params.credential.base_url.trim_end_matches('/'));
    let response = reqwest::Client::new()
        .post(&url)
        .bearer_auth(&params.credential.token)
        .header("x-session-id", &params.credential.session_id)
        .header(GATEWAY_CONVERSATION_ID, params.conversation_id)
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

    let json: Value = serde_json::from_str(&payload)
        .inspect_err(|e| tracing::warn!(error = %e, "eval gateway response was not JSON"))
        .ok()?;

    let text = super::extract::assistant_answer(Some(&json))?;
    Some(GatewayAnswer {
        text,
        conversation_id: params.conversation_id.to_owned(),
    })
}

/// Pull a JSON object out of a model reply that may be fenced or prefaced.
/// The gateway is a passthrough, so there is no structured-output enforcement
/// to lean on and the reply has to be read defensively.
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
