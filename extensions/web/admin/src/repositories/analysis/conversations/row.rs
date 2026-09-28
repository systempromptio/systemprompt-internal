//! The conversation row shared by the Analysis pages.
//!
//! It is the `conversation_facts` record plus the judge's columns. The JSON
//! wire form keeps the context id as text; the default `Id` is the typed
//! `ContextId` a single-row read decodes directly.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt::identifiers::{ContextId, SessionId, UserId};
use systemprompt_web_shared::{GroupId, ProjectId};

/// One conversation as the pages show it: the fact row, the judge's label
/// and the tokens of its last turns for the row sparkline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationFactRow<Id = ContextId> {
    pub context_id: Id,
    pub title: String,
    pub user_id: UserId,
    pub display_name: Option<String>,
    pub session_id: Option<SessionId>,
    pub client_session_id: Option<String>,
    pub group_id: Option<GroupId>,
    pub project_id: Option<ProjectId>,
    pub group_name: Option<String>,
    pub project_name: Option<String>,
    pub client_kind: String,
    pub client_attestation: String,
    pub wire_protocol: String,
    pub model: Option<String>,
    pub provider: Option<String>,
    pub models: Vec<String>,
    pub providers: Vec<String>,
    pub request_count: i64,
    pub turn_count: i64,
    pub side_call_count: i64,
    pub side_call_cost_microdollars: i64,
    pub error_count: i64,
    pub rejected_count: i64,
    pub streaming_count: i64,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cache_read_tokens: i64,
    pub cache_creation_tokens: i64,
    pub reasoning_tokens: i64,
    pub total_tokens: i64,
    pub cost_microdollars: i64,
    pub p50_latency_ms: Option<i32>,
    pub p95_latency_ms: Option<i32>,
    pub max_latency_ms: Option<i32>,
    pub tool_calls_intended: i64,
    pub tool_calls_executed: i64,
    pub tool_calls_failed: i64,
    pub artifact_count: i64,
    #[serde(default)]
    pub artifact_files: i64,
    #[serde(default)]
    pub artifact_cards: i64,
    pub safety_findings: i64,
    pub safety_blocked: i64,
    pub gov_allow: i64,
    pub gov_warn: i64,
    pub gov_deny: i64,
    pub prompt_count: i64,
    pub hook_event_count: i64,
    pub hook_status: Option<String>,
    pub skill_invocations: i64,
    pub skills: Vec<String>,
    pub first_at: DateTime<Utc>,
    pub last_at: DateTime<Utc>,
    pub duration_seconds: i64,
    #[serde(default)]
    pub active_ms: i64,
    pub judge_status: Option<String>,
    pub judge_title: Option<String>,
    pub category: Option<String>,
    pub summary: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub skills_used: Vec<String>,
    pub outcome: Option<String>,
    pub completion: Option<i16>,
    pub completion_rationale: Option<String>,
    pub confidence: Option<f32>,
    pub classified_at: Option<DateTime<Utc>>,
    pub judge_model: Option<String>,
    pub judge_cost_microdollars: Option<i64>,
    #[serde(default)]
    pub judge_tokens: i64,
    pub judge_trigger: Option<String>,
    #[serde(default)]
    pub turn_tokens: Vec<i64>,
    #[serde(flatten)]
    pub continuation: ContinuationLink,
}

/// A Claude Code session resumed after compaction, and the conversation it
/// most likely continues (text, since it only builds a link).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContinuationLink {
    #[serde(default)]
    pub is_continuation: bool,
    #[serde(default)]
    pub prev_context_id: Option<String>,
    #[serde(default)]
    pub prev_title: Option<String>,
}
