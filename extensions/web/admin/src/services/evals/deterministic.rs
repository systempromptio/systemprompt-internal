//! The zero-cost pre-pass.
//!
//! Runs before any judge call. Two jobs: attach cheap flags that are true by
//! inspection rather than by opinion, and short-circuit the items where a
//! judge has nothing to grade (a failed request, an empty answer). Those come
//! back as `skipped` or `fail` and never reach the model, which is most of the
//! cost saving on a noisy window.

use crate::repositories::evals::EvalVerdict;
use crate::repositories::evals::sampling::EvalCandidate;

use super::extract;

/// Phrases that mark a bare refusal. Deliberately narrow: a caveat inside a
/// real answer is not a refusal, and the judge grades that better than a
/// substring match can.
const REFUSAL_MARKERS: [&str; 6] = [
    "i can't help with",
    "i cannot help with",
    "i'm not able to help",
    "i am not able to help",
    "i can't assist with",
    "i cannot assist with",
];

/// Answers longer than this are flagged verbose; the judge decides whether it
/// mattered.
const VERBOSE_CHARS: usize = 12_000;

#[derive(Debug, Clone)]
pub struct PrePass {
    pub flags: Vec<String>,
    /// Set when the item is decided without a judge call.
    pub short_circuit: Option<(EvalVerdict, String)>,
    pub prompt: Option<String>,
    pub answer: Option<String>,
}

#[must_use]
pub fn run_pre_pass(candidate: &EvalCandidate) -> PrePass {
    let prompt = extract::final_user_prompt(candidate.request_body.as_ref())
        .or_else(|| candidate.request_excerpt.clone());
    let answer = extract::assistant_answer(candidate.response_body.as_ref())
        .or_else(|| candidate.response_excerpt.clone());

    let mut flags = Vec::new();

    let request_failed = !matches!(
        candidate.status.as_str(),
        "completed" | "pending" | "streaming"
    );

    if candidate.response_truncated
        || extract::stop_reason(candidate.response_body.as_ref()).as_deref() == Some("max_tokens")
    {
        flags.push("truncated".to_owned());
    }

    let tool_calls = extract::tool_use_count(candidate.response_body.as_ref());
    let answer_text = answer.as_deref().unwrap_or_default().trim();

    if answer_text.is_empty() && tool_calls == 0 {
        flags.push("empty".to_owned());
    }

    let lowered = answer_text.to_lowercase();
    if REFUSAL_MARKERS.iter().any(|m| lowered.contains(m)) {
        flags.push("refusal".to_owned());
    }

    if answer_text.chars().count() > VERBOSE_CHARS {
        flags.push("verbose".to_owned());
    }

    let short_circuit = if request_failed {
        let reason = candidate
            .error_message
            .clone()
            .unwrap_or_else(|| format!("Request ended with status '{}'.", candidate.status));
        Some((
            EvalVerdict::Fail,
            format!("Request never produced an answer. {reason}"),
        ))
    } else if answer_text.is_empty() && tool_calls == 0 {
        Some((
            EvalVerdict::Fail,
            "Response body contained no assistant text and no tool calls.".to_owned(),
        ))
    } else if prompt.as_deref().unwrap_or_default().trim().is_empty() {
        Some((
            EvalVerdict::Skipped,
            "No user prompt was recoverable from the stored request body.".to_owned(),
        ))
    } else {
        None
    };

    PrePass {
        flags,
        short_circuit,
        prompt,
        answer,
    }
}
