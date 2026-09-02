//! Live proof that the categorization contract survives every provider wire.
//!
//! Each test builds the exact request the categorization job builds — the
//! real system prompt, the real derived schema, `strict: true` — pushes it
//! through core's wire codec for one provider, sends it to that provider's
//! real API on its cheapest model, parses the reply with core's codec, and
//! hands the content to the job's own `parse_output`. If the codec shapes the
//! schema wrongly for a provider, or the provider is not actually constrained
//! to it, this is where it fails — not on the brain@ backlog at 09:15.
//!
//! The tests are `#[ignore]` and run only when `SYSTEMPROMPT_LIVE_SECRETS`
//! names a profile `secrets.json` carrying `anthropic`, `openai` and `gemini`
//! keys. Models default to the cheapest tier and can be overridden with
//! `LIVE_ANTHROPIC_MODEL`, `LIVE_OPENAI_MODEL`, `LIVE_GEMINI_MODEL`.

use std::collections::HashMap;

use reqwest::Client;
use serde_json::Value;
use systemprompt_knowledge_jobs::internals::{
    Categorization, parse_output, response_schema, system_prompt, user_prompt,
};
use systemprompt_models::wire::canonical::{
    CanonicalContent, CanonicalMessage, CanonicalRequest, CanonicalResponse, ResponseFormat, Role,
};
use systemprompt_models::wire::{anthropic, gemini, openai_chat};

const SCHEMA_NAME: &str = "knowledge_categorization";

struct LiveSecrets {
    anthropic: String,
    openai: String,
    gemini: String,
}

fn live_secrets() -> Option<LiveSecrets> {
    let path = std::env::var("SYSTEMPROMPT_LIVE_SECRETS").ok()?;
    let raw = std::fs::read_to_string(path).ok()?;
    let file: Value = serde_json::from_str(&raw).ok()?;
    let key = |name: &str| file.get(name)?.as_str().map(str::to_owned);
    Some(LiveSecrets {
        anthropic: key("anthropic")?,
        openai: key("openai")?,
        gemini: key("gemini")?,
    })
}

fn model(env: &str, default: &str) -> String {
    std::env::var(env).unwrap_or_else(|_| default.to_owned())
}

const RICH_THREAD: &str = "From: Dimitri Sidney <dimitri.sidney@customertimes.com>\n\
To: ed@systemprompt.io\nCc: roman@martynenko.co\nDate: 2026-08-31T18:02:38Z\n\n\
Hi Ed,\n\nCan you share two slides (one - what your system does, two - dashboard with example)? \
I have a prospect customer who is interested exactly in this function; they want to decide \
before the end of September and the budget is around 40k EUR for the first year.\n\n\
Roman, can you set up the intro call for next Tuesday?\n\nRegards,\nDimitri";

const TRIVIAL_THREAD: &str =
    "From: Edward Burton <ed@systemprompt.io>\nTo: brain@systemprompt.io\n\nThis is a test email";

fn request(model: &str, title: &str, body: &str) -> CanonicalRequest {
    CanonicalRequest {
        model: model.to_owned(),
        system: Some(system_prompt()),
        messages: vec![CanonicalMessage {
            role: Role::User,
            content: vec![CanonicalContent::Text(user_prompt(title, body))],
        }],
        max_tokens: 4096,
        response_format: Some(ResponseFormat::JsonSchema {
            name: SCHEMA_NAME.to_owned(),
            schema: response_schema(),
            strict: true,
        }),
        ..CanonicalRequest::default()
    }
}

async fn post(
    client: &Client,
    url: &str,
    headers: &[(&str, String)],
    body: &Value,
) -> Result<Value, String> {
    let mut req = client.post(url).json(body);
    for (name, value) in headers {
        req = req.header(*name, value);
    }
    let response = req.send().await.map_err(|e| format!("transport: {e}"))?;
    let status = response.status();
    let text = response.text().await.map_err(|e| format!("body: {e}"))?;
    if !status.is_success() {
        return Err(format!("{url} returned {status}: {text}"));
    }
    serde_json::from_str(&text).map_err(|e| format!("non-JSON reply: {e}: {text}"))
}

fn tool_input(parsed: &CanonicalResponse) -> Option<String> {
    parsed.content.iter().find_map(|block| match block {
        CanonicalContent::ToolUse { input, .. } => Some(input.to_string()),
        _ => None,
    })
}

fn text(parsed: &CanonicalResponse) -> String {
    parsed
        .content
        .iter()
        .filter_map(|block| match block {
            CanonicalContent::Text(t) => Some(t.as_str()),
            _ => None,
        })
        .collect()
}

fn assert_categorized(provider: &str, title: &str, content: &str) -> Categorization {
    let parsed = parse_output(content)
        .unwrap_or_else(|e| panic!("{provider} / {title}: {e}\nraw: {content}"));
    assert!(
        !parsed.summary.trim().is_empty(),
        "{provider} / {title}: empty summary"
    );
    parsed
}

async fn anthropic_round_trip(
    client: &Client,
    key: &str,
    model: &str,
    title: &str,
    body: &str,
) -> Categorization {
    let req = request(model, title, body);
    let wire = anthropic::build_request_body(&req, model, None);
    assert_eq!(
        wire["tools"][0]["strict"],
        Value::Bool(true),
        "the forced tool must be strict on the wire"
    );
    let headers: Vec<(&str, String)> = anthropic::auth_headers(key).to_vec();
    let value = post(
        client,
        "https://api.anthropic.com/v1/messages",
        &headers,
        &wire,
    )
    .await
    .unwrap_or_else(|e| panic!("anthropic / {title}: {e}"));
    let parsed = anthropic::parse_response(&value, model);
    let content = tool_input(&parsed)
        .unwrap_or_else(|| panic!("anthropic / {title}: no tool_use block in {value}"));
    assert_categorized("anthropic", title, &content)
}

async fn openai_round_trip(
    client: &Client,
    key: &str,
    model: &str,
    title: &str,
    body: &str,
) -> Categorization {
    let req = request(model, title, body);
    let wire = openai_chat::build_request_body(&req, model, None);
    assert_eq!(
        wire["response_format"]["json_schema"]["strict"],
        Value::Bool(true)
    );
    let value = post(
        client,
        "https://api.openai.com/v1/chat/completions",
        &[("authorization", format!("Bearer {key}"))],
        &wire,
    )
    .await
    .unwrap_or_else(|e| panic!("openai / {title}: {e}"));
    let parsed = openai_chat::parse_response(&value, model);
    assert_categorized("openai", title, &text(&parsed))
}

async fn gemini_round_trip(
    client: &Client,
    key: &str,
    model: &str,
    title: &str,
    body: &str,
) -> Categorization {
    let req = request(model, title, body);
    let wire = gemini::build_request_body(&req, None);
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta{}",
        gemini::upstream_path(model, false)
    );
    let value = post(
        client,
        &url,
        &[(gemini::API_KEY_HEADER, key.to_owned())],
        &wire,
    )
    .await
    .unwrap_or_else(|e| panic!("gemini / {title}: {e}"));
    let parsed = gemini::parse_response(&value, model);
    assert_categorized("gemini", title, &text(&parsed))
}

fn fixtures() -> HashMap<&'static str, &'static str> {
    HashMap::from([
        ("Re: request for info", RICH_THREAD),
        ("Test", TRIVIAL_THREAD),
    ])
}

#[tokio::test]
#[ignore = "live provider call; set SYSTEMPROMPT_LIVE_SECRETS"]
async fn anthropic_wire_yields_a_schema_valid_categorization() {
    let Some(secrets) = live_secrets() else {
        return;
    };
    let client = Client::new();
    let model = model("LIVE_ANTHROPIC_MODEL", "claude-haiku-4-5-20251001");
    for (title, body) in fixtures() {
        let c = anthropic_round_trip(&client, &secrets.anthropic, &model, title, body).await;
        if title == "Re: request for info" {
            assert!(!c.crm_intent.tasks.is_empty(), "rich thread carries tasks");
        }
    }
}

#[tokio::test]
#[ignore = "live provider call; set SYSTEMPROMPT_LIVE_SECRETS"]
async fn openai_wire_yields_a_schema_valid_categorization() {
    let Some(secrets) = live_secrets() else {
        return;
    };
    let client = Client::new();
    let model = model("LIVE_OPENAI_MODEL", "gpt-4o-mini");
    for (title, body) in fixtures() {
        let c = openai_round_trip(&client, &secrets.openai, &model, title, body).await;
        if title == "Re: request for info" {
            assert!(!c.crm_intent.tasks.is_empty(), "rich thread carries tasks");
        }
    }
}

#[tokio::test]
#[ignore = "live provider call; set SYSTEMPROMPT_LIVE_SECRETS"]
async fn gemini_wire_yields_a_schema_valid_categorization() {
    let Some(secrets) = live_secrets() else {
        return;
    };
    let client = Client::new();
    let model = model("LIVE_GEMINI_MODEL", "gemini-2.5-flash");
    for (title, body) in fixtures() {
        let c = gemini_round_trip(&client, &secrets.gemini, &model, title, body).await;
        if title == "Re: request for info" {
            assert!(!c.crm_intent.tasks.is_empty(), "rich thread carries tasks");
        }
    }
}
