-- The deterministic conversation record: `conversation_facts`,
-- `conversation_skill_facts` and the rollup watermark. Twin of
-- schema/45_conversation_facts.sql in its final shape (artifact split and
-- `active_ms` included). The refresh functions are declarative and applied
-- before migrations run; the `ai_requests(updated_at)` index is declarative
-- because the table is core's.

CREATE TABLE IF NOT EXISTS conversation_facts (
    context_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    session_id TEXT,
    client_session_id TEXT,
    group_id TEXT,
    project_id TEXT,
    client_kind TEXT NOT NULL DEFAULT 'unknown',
    client_attestation TEXT NOT NULL DEFAULT 'unknown',
    wire_protocol TEXT NOT NULL DEFAULT 'unknown',
    model TEXT,
    provider TEXT,
    models TEXT[] NOT NULL DEFAULT '{}',
    providers TEXT[] NOT NULL DEFAULT '{}',
    request_count BIGINT NOT NULL DEFAULT 0,
    turn_count BIGINT NOT NULL DEFAULT 0,
    side_call_count BIGINT NOT NULL DEFAULT 0,
    side_call_cost_microdollars BIGINT NOT NULL DEFAULT 0,
    error_count BIGINT NOT NULL DEFAULT 0,
    rejected_count BIGINT NOT NULL DEFAULT 0,
    streaming_count BIGINT NOT NULL DEFAULT 0,
    input_tokens BIGINT NOT NULL DEFAULT 0,
    output_tokens BIGINT NOT NULL DEFAULT 0,
    cache_read_tokens BIGINT NOT NULL DEFAULT 0,
    cache_creation_tokens BIGINT NOT NULL DEFAULT 0,
    reasoning_tokens BIGINT NOT NULL DEFAULT 0,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    p50_latency_ms INTEGER,
    p95_latency_ms INTEGER,
    max_latency_ms INTEGER,
    active_ms BIGINT NOT NULL DEFAULT 0,
    tool_calls_intended BIGINT NOT NULL DEFAULT 0,
    tool_calls_executed BIGINT NOT NULL DEFAULT 0,
    tool_calls_failed BIGINT NOT NULL DEFAULT 0,
    artifact_count BIGINT NOT NULL DEFAULT 0,
    artifact_files BIGINT NOT NULL DEFAULT 0,
    artifact_cards BIGINT NOT NULL DEFAULT 0,
    safety_findings BIGINT NOT NULL DEFAULT 0,
    safety_blocked BIGINT NOT NULL DEFAULT 0,
    gov_allow BIGINT NOT NULL DEFAULT 0,
    gov_warn BIGINT NOT NULL DEFAULT 0,
    gov_deny BIGINT NOT NULL DEFAULT 0,
    prompt_count BIGINT NOT NULL DEFAULT 0,
    hook_event_count BIGINT NOT NULL DEFAULT 0,
    hook_status TEXT,
    skill_invocations BIGINT NOT NULL DEFAULT 0,
    skills TEXT[] NOT NULL DEFAULT '{}',
    first_at TIMESTAMPTZ NOT NULL,
    last_at TIMESTAMPTZ NOT NULL,
    duration_seconds BIGINT NOT NULL DEFAULT 0,
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_conversation_facts_last_at ON conversation_facts(last_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_facts_user ON conversation_facts(user_id, last_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_facts_group ON conversation_facts(group_id, last_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_facts_project ON conversation_facts(project_id, last_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_facts_model ON conversation_facts(model, last_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_facts_client ON conversation_facts(client_kind, last_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_facts_session ON conversation_facts(client_session_id)
    WHERE client_session_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_conversation_facts_skills ON conversation_facts USING gin(skills);

CREATE TABLE IF NOT EXISTS conversation_skill_facts (
    context_id TEXT NOT NULL REFERENCES conversation_facts(context_id) ON DELETE CASCADE,
    plugin_id TEXT NOT NULL,
    skill TEXT NOT NULL,
    user_id TEXT NOT NULL,
    marketplace_id TEXT,
    marketplace_hash TEXT,
    invocations BIGINT NOT NULL DEFAULT 0,
    failures BIGINT NOT NULL DEFAULT 0,
    first_invoked_at TIMESTAMPTZ NOT NULL,
    last_invoked_at TIMESTAMPTZ NOT NULL,
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY (context_id, plugin_id, skill)
);

CREATE INDEX IF NOT EXISTS idx_conversation_skill_facts_version
    ON conversation_skill_facts(marketplace_id, marketplace_hash, first_invoked_at);

CREATE INDEX IF NOT EXISTS idx_conversation_skill_facts_first
    ON conversation_skill_facts(first_invoked_at);

CREATE TABLE IF NOT EXISTS conversation_rollup_state (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    watermark TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
