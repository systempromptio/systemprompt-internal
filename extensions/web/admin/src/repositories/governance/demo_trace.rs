//! The demo trace: one ordered story per agent session.
//!
//! Three tables record what a governed coding agent did, and each answers a
//! different question:
//!
//! * `governance_decisions` — what was asked for, and whether policy allowed it
//! * `ai_requests` — what actually reached a provider, and what it cost
//! * `plugin_usage_events` — which tool calls ran to completion
//!
//! Read separately they are three lists. Read as one time-ordered union they
//! are the demo: a prompt denied by `secret_scan` sits immediately above the
//! `ai_requests` row that never happened, which is the whole point.

use sqlx::PgPool;
use systemprompt::identifiers::{AgentId, SessionId};

/// One agent session that produced governance decisions.
#[derive(Debug, Clone)]
pub struct DemoSessionRow {
    pub session_id: SessionId,
    pub allowed: i64,
    pub denied: i64,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub last_at: chrono::DateTime<chrono::Utc>,
}

/// One event in the merged timeline.
#[derive(Debug, Clone)]
pub struct DemoTraceRow {
    pub at: chrono::DateTime<chrono::Utc>,
    /// `prompt` | `tool` | `request` | `fire`
    pub kind: String,
    /// What was attempted: a tool name, a model id, or `user_prompt`.
    pub subject: String,
    /// `allow` | `deny` | the request status | `ok`
    pub outcome: String,
    /// Governing policy, where one applies.
    pub policy: String,
    pub detail: String,
}

/// Sessions with governance activity for one agent, newest first.
pub async fn list_demo_sessions(
    pool: &PgPool,
    agent_id: &AgentId,
    limit: i64,
) -> Result<Vec<DemoSessionRow>, sqlx::Error> {
    sqlx::query_as!(
        DemoSessionRow,
        r#"SELECT session_id as "session_id!: _",
                  COUNT(*) FILTER (WHERE decision = 'allow') as "allowed!",
                  COUNT(*) FILTER (WHERE decision = 'deny')  as "denied!",
                  MIN(created_at) as "started_at!",
                  MAX(created_at) as "last_at!"
           FROM governance_decisions
           WHERE agent_id = $1
           GROUP BY session_id
           ORDER BY MAX(created_at) DESC
           LIMIT $2"#,
        agent_id.as_str(),
        limit,
    )
    .fetch_all(pool)
    .await
}

/// The merged, time-ordered trace for one session.
pub async fn list_demo_trace(
    pool: &PgPool,
    session_id: &SessionId,
    limit: i64,
) -> Result<Vec<DemoTraceRow>, sqlx::Error> {
    sqlx::query_as!(
        DemoTraceRow,
        r#"SELECT created_at as "at!", kind as "kind!", subject as "subject!",
                  outcome as "outcome!", policy as "policy!", detail as "detail!"
           FROM (
             SELECT created_at,
                    CASE WHEN tool_name = 'user_prompt' THEN 'prompt' ELSE 'tool' END as kind,
                    tool_name as subject,
                    decision as outcome,
                    policy,
                    reason as detail
             FROM governance_decisions
             WHERE session_id = $1
             UNION ALL
             SELECT created_at,
                    'request' as kind,
                    COALESCE(requested_model, model) as subject,
                    status as outcome,
                    '' as policy,
                    COALESCE(error_message,
                             'tokens ' || COALESCE(input_tokens, 0)::text || ' in / '
                                       || COALESCE(output_tokens, 0)::text || ' out') as detail
             FROM ai_requests
             WHERE session_id = $1
             UNION ALL
             SELECT created_at,
                    'fire' as kind,
                    COALESCE(tool_name, event_type) as subject,
                    'ok' as outcome,
                    '' as policy,
                    COALESCE(description, event_type) as detail
             FROM plugin_usage_events
             WHERE session_id = $1
           ) trace
           ORDER BY created_at ASC
           LIMIT $2"#,
        session_id.as_str(),
        limit,
    )
    .fetch_all(pool)
    .await
}
