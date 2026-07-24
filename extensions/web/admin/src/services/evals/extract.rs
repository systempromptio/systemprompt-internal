//! Pulling readable text out of stored gateway payloads.
//!
//! `ai_request_payloads.request_body` is whatever the client POSTed to
//! `/v1/messages` and `response_body` is what came back, so both are
//! Anthropic-shaped: `content` is either a bare string or a list of typed
//! blocks. Everything here degrades to the stored excerpt rather than failing,
//! because a payload we cannot parse is still worth flagging.

use serde_json::Value;

/// The final user turn — what the assistant was actually answering.
#[must_use]
pub fn final_user_prompt(request_body: Option<&Value>) -> Option<String> {
    let messages = request_body?.get("messages")?.as_array()?;
    messages
        .iter()
        .rev()
        .find(|m| m.get("role").and_then(Value::as_str) == Some("user"))
        .and_then(|m| m.get("content"))
        .map(flatten_content)
        .filter(|s| !s.trim().is_empty())
}

/// The system prompt the client sent, if any. Included in the judge's view so
/// it can tell an instruction the user gave from one the harness gave.
#[must_use]
pub fn system_prompt(request_body: Option<&Value>) -> Option<String> {
    let system = request_body?.get("system")?;
    let text = flatten_content(system);
    (!text.trim().is_empty()).then_some(text)
}

/// The assistant's answer text, tool calls flattened into a readable form.
#[must_use]
pub fn assistant_answer(response_body: Option<&Value>) -> Option<String> {
    let body = response_body?;
    let content = body.get("content").or_else(|| body.get("completion"))?;
    let text = flatten_content(content);
    (!text.trim().is_empty()).then_some(text)
}

/// How many tool-use blocks the answer contains. A non-zero count with no text
/// is a normal agentic turn, not an empty answer.
#[must_use]
pub fn tool_use_count(response_body: Option<&Value>) -> usize {
    response_body
        .and_then(|b| b.get("content"))
        .and_then(Value::as_array)
        .map_or(0, |blocks| {
            blocks
                .iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("tool_use"))
                .count()
        })
}

/// The provider's own stop reason, when it recorded one.
#[must_use]
pub fn stop_reason(response_body: Option<&Value>) -> Option<String> {
    response_body?
        .get("stop_reason")?
        .as_str()
        .map(str::to_owned)
}

/// Collapse Anthropic content — a string, or a list of typed blocks — into
/// plain text. Tool calls become a readable one-liner so the judge can see the
/// assistant acted rather than seeing a blank answer.
fn flatten_content(content: &Value) -> String {
    match content {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(flatten_block)
            .collect::<Vec<_>>()
            .join("\n"),
        other => other.to_string(),
    }
}

fn flatten_block(block: &Value) -> Option<String> {
    match block.get("type").and_then(Value::as_str) {
        Some("text") => block
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|s| !s.is_empty()),
        Some("tool_use") => {
            let name = block
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("(unnamed)");
            let input = block
                .get("input")
                .map(ToString::to_string)
                .unwrap_or_default();
            Some(format!("[tool_use {name}] {input}"))
        },
        Some("tool_result") => {
            let inner = block.get("content").map(flatten_content).unwrap_or_default();
            Some(format!("[tool_result] {inner}"))
        },
        Some("thinking") => None,
        _ => block
            .get("text")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|s| !s.is_empty()),
    }
}

/// Cap text handed to the judge. Long transcripts cost judge tokens without
/// improving the verdict, and the tail is where the answer actually is.
#[must_use]
pub fn truncate_for_judge(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let head: String = text.chars().take(max_chars / 2).collect();
    let tail: String = text
        .chars()
        .skip(text.chars().count() - max_chars / 2)
        .collect();
    format!("{head}\n\n[… {} characters elided …]\n\n{tail}", {
        text.chars().count() - max_chars
    })
}

/// Short single-line excerpt for the results table.
#[must_use]
pub fn excerpt(text: &str, max_chars: usize) -> String {
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if collapsed.chars().count() <= max_chars {
        collapsed
    } else {
        let head: String = collapsed.chars().take(max_chars).collect();
        format!("{head}…")
    }
}
