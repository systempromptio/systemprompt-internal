//! The judge call itself.
//!
//! One gateway `/v1/messages` call per item (see [`super::gateway_client`]),
//! parsed into the typed verdict in [`super::rubric`]. Because the call goes
//! through our own gateway it lands in `ai_requests` like any other client's
//! traffic, which is how the per-run judge cost is a recorded number rather
//! than an estimate.

use sqlx::PgPool;
use systemprompt::identifiers::UserId;

use super::gateway_client::{self, CallParams, GatewayCredential};
use super::rubric::{
    JUDGE_SYSTEM_PROMPT, JudgeVerdict, PAIRWISE_SYSTEM_PROMPT, PairwiseVerdict, judge_user_prompt,
    pairwise_user_prompt,
};

/// Output-token budget for a judge call. The verdict is a small JSON object; a
/// larger budget only pays for rambling rationales.
const JUDGE_MAX_TOKENS: u32 = 2048;

/// Which model grades, under whose credential, for which run.
#[derive(Debug, Clone)]
pub(crate) struct JudgeConfig {
    pub provider: String,
    pub model: String,
    pub actor_user_id: UserId,
    pub run_id: String,
    pub credential: GatewayCredential,
}

/// A verdict plus what it cost to obtain.
#[derive(Debug, Clone)]
pub(crate) struct JudgedItem {
    pub verdict: JudgeVerdict,
    pub cost_microdollars: i64,
}

/// Score one prompt/answer pair.
pub(crate) async fn judge_answer(
    pool: &PgPool,
    config: &JudgeConfig,
    prompt: &str,
    answer: &str,
) -> Option<JudgedItem> {
    let raw = call_judge(
        config,
        JUDGE_SYSTEM_PROMPT,
        &judge_user_prompt(prompt, answer),
    )
    .await?;

    let verdict = parse_reply::<JudgeVerdict>(&raw.text, "judge")?.normalised();
    let cost = lookup_cost(pool, &raw.conversation_id).await;

    Some(JudgedItem {
        verdict,
        cost_microdollars: cost,
    })
}

/// A pairwise decision plus what it cost.
#[derive(Debug, Clone)]
pub(crate) struct JudgedPair {
    pub verdict: PairwiseVerdict,
    pub cost_microdollars: i64,
}

#[derive(Debug)]
pub(crate) struct PairParams<'a> {
    pub pool: &'a PgPool,
    pub config: &'a JudgeConfig,
    pub prompt: &'a str,
    pub answer_a: &'a str,
    pub answer_b: &'a str,
}

/// Compare two answers to the same prompt. Callers run this twice with the
/// answers swapped; see [`super::pairwise`].
pub(crate) async fn judge_pair(params: PairParams<'_>) -> Option<JudgedPair> {
    let raw = call_judge(
        params.config,
        PAIRWISE_SYSTEM_PROMPT,
        &pairwise_user_prompt(params.prompt, params.answer_a, params.answer_b),
    )
    .await?;

    let verdict = parse_reply::<PairwiseVerdict>(&raw.text, "pairwise")?;
    let cost = lookup_cost(params.pool, &raw.conversation_id).await;

    Some(JudgedPair {
        verdict,
        cost_microdollars: cost,
    })
}

async fn call_judge(
    config: &JudgeConfig,
    system: &str,
    user: &str,
) -> Option<gateway_client::GatewayAnswer> {
    gateway_client::call_messages(CallParams {
        credential: &config.credential,
        model: &config.model,
        system: Some(system),
        user,
        max_tokens: JUDGE_MAX_TOKENS,
        // Why: the run id rides along so a run's judge calls can be found in
        // `ai_requests` by conversation id alone.
        conversation_id: &format!("{}-{}", config.run_id, super::new_id("judge")),
    })
    .await
}

/// The gateway is a passthrough, so a reply can arrive fenced or prefaced; the
/// object is carved out before parsing rather than trusting the shape.
fn parse_reply<T: serde::de::DeserializeOwned>(text: &str, what: &str) -> Option<T> {
    let json = gateway_client::extract_json_object(text).unwrap_or(text);
    serde_json::from_str::<T>(json)
        .inspect_err(|e| {
            tracing::warn!(
                error = %e,
                kind = what,
                reply = %text.chars().take(300).collect::<String>(),
                "eval judge returned an unparseable verdict"
            );
        })
        .ok()
}

/// The judge's own request row carries the authoritative cost, found by the
/// conversation id the call was tagged with. No row means the gateway has not
/// written it yet; report zero rather than guessing.
async fn lookup_cost(pool: &PgPool, conversation_id: &str) -> i64 {
    crate::repositories::evals::sampling::find_conversation_cost(pool, conversation_id)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "could not read judge request cost");
            None
        })
        .unwrap_or(0)
}
