//! The judge call itself.
//!
//! One `AiService::generate` per item, structured output forced against the
//! schema in [`super::rubric`]. Judge calls go through the same AI service as
//! everything else, so each one lands in `ai_requests` and is itself audited
//! and costed — which is how [`judge_cost_microdollars`] can be a real number
//! rather than an estimate.

use std::sync::Arc;

use sqlx::PgPool;
use systemprompt::ai::{AiMessage, AiRequest, AiService, StructuredOutputOptions};
use systemprompt::identifiers::{AgentName, ContextId, SessionId, TraceId, UserId};
use systemprompt::models::auth::{AuthenticatedUser, UserType};
use systemprompt::models::execution::context::RequestContext;

use super::rubric::{
    JUDGE_SYSTEM_PROMPT, JudgeVerdict, PAIRWISE_SYSTEM_PROMPT, PairwiseVerdict,
    judge_user_prompt, judge_verdict_schema, pairwise_user_prompt, pairwise_verdict_schema,
};

/// Max output tokens for a judge call. The verdict is a small JSON object; a
/// large budget here only pays for rambling rationales.
const JUDGE_MAX_TOKENS: u32 = 2048;

/// Which model does the grading, and on whose authority.
#[derive(Debug, Clone)]
pub struct JudgeConfig {
    pub provider: String,
    pub model: String,
    pub actor_user_id: UserId,
    pub run_id: String,
}

impl JudgeConfig {
    /// Default judge: the configured default provider/model. Callers override
    /// when the population being judged uses that same model.
    #[must_use]
    pub fn from_defaults(ai: &Arc<AiService>, actor_user_id: UserId, run_id: String) -> Self {
        Self {
            provider: ai.default_provider().to_owned(),
            model: ai.default_model().to_owned(),
            actor_user_id,
            run_id,
        }
    }

    fn request_context(&self) -> RequestContext {
        self.context_for("eval-judge")
    }

    /// Context for a replayed answer. Named after the model under test so the
    /// audit trail distinguishes "we asked this model something" from "we
    /// graded something".
    pub(super) fn replay_context(&self, model: &str) -> RequestContext {
        self.context_for(&format!("eval-replay:{model}"))
    }

    fn context_for(&self, agent: &str) -> RequestContext {
        RequestContext::new(
            SessionId::new(format!("eval-{}", self.run_id)),
            TraceId::new(uuid::Uuid::new_v4().to_string()),
            ContextId::new(""),
            AgentName::new(agent),
        )
        .with_user(AuthenticatedUser::new(
            self.actor_user_id
                .as_str()
                .parse()
                .unwrap_or_else(|_| uuid::Uuid::nil()),
            self.actor_user_id.as_str().to_owned(),
            String::new(),
            Vec::new(),
        ))
        .with_user_type(UserType::User)
    }
}

/// A verdict plus what it cost to obtain.
#[derive(Debug, Clone)]
pub struct JudgedItem {
    pub verdict: JudgeVerdict,
    pub cost_microdollars: i64,
}

/// Score one prompt/answer pair.
pub async fn judge_answer(
    ai: &Arc<AiService>,
    pool: &PgPool,
    config: &JudgeConfig,
    prompt: &str,
    answer: &str,
) -> Option<JudgedItem> {
    let messages = vec![
        AiMessage::system(JUDGE_SYSTEM_PROMPT),
        AiMessage::user(judge_user_prompt(prompt, answer)),
    ];

    let request = AiRequest::builder(
        messages,
        &config.provider,
        &config.model,
        JUDGE_MAX_TOKENS,
        config.request_context(),
    )
    .with_structured_output(StructuredOutputOptions::with_schema(judge_verdict_schema()))
    .build();

    let response = ai
        .generate(&request)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "eval judge call failed"))
        .ok()?;

    let verdict = serde_json::from_str::<JudgeVerdict>(&response.content)
        .inspect_err(|e| tracing::warn!(error = %e, "eval judge returned unparseable verdict"))
        .ok()?
        .normalised();

    let cost = lookup_cost(pool, &response.request_id.to_string()).await;

    Some(JudgedItem {
        verdict,
        cost_microdollars: cost,
    })
}

/// A pairwise decision plus what it cost.
#[derive(Debug, Clone)]
pub struct JudgedPair {
    pub verdict: PairwiseVerdict,
    pub cost_microdollars: i64,
}

/// Compare two answers to the same prompt. Callers run this twice with the
/// answers swapped; see [`super::pairwise`].
pub async fn judge_pair(
    ai: &Arc<AiService>,
    pool: &PgPool,
    config: &JudgeConfig,
    prompt: &str,
    answer_a: &str,
    answer_b: &str,
) -> Option<JudgedPair> {
    let messages = vec![
        AiMessage::system(PAIRWISE_SYSTEM_PROMPT),
        AiMessage::user(pairwise_user_prompt(prompt, answer_a, answer_b)),
    ];

    let request = AiRequest::builder(
        messages,
        &config.provider,
        &config.model,
        JUDGE_MAX_TOKENS,
        config.request_context(),
    )
    .with_structured_output(StructuredOutputOptions::with_schema(
        pairwise_verdict_schema(),
    ))
    .build();

    let response = ai
        .generate(&request)
        .await
        .inspect_err(|e| tracing::warn!(error = %e, "eval pairwise call failed"))
        .ok()?;

    let verdict = serde_json::from_str::<PairwiseVerdict>(&response.content)
        .inspect_err(|e| tracing::warn!(error = %e, "eval pairwise returned unparseable verdict"))
        .ok()?;

    let cost = lookup_cost(pool, &response.request_id.to_string()).await;

    Some(JudgedPair {
        verdict,
        cost_microdollars: cost,
    })
}

/// The judge's own request row carries the authoritative cost. Absent row
/// means the provider call was not recorded yet; report zero rather than
/// guessing.
async fn lookup_cost(pool: &PgPool, request_id: &str) -> i64 {
    crate::repositories::evals::sampling::find_request_cost(pool, request_id)
        .await
        .unwrap_or_else(|e| {
            tracing::debug!(error = %e, "could not read judge request cost");
            None
        })
        .unwrap_or(0)
}
