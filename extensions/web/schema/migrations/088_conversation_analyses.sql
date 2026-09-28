-- The judge's one label per gateway conversation. Twin of
-- schema/37_conversation_analyses.sql, in its final shape: astound grew it
-- over three migrations (table, judge columns, completion index); this
-- instance never had it, so it lands once. The `conversation_skill_uses`
-- view is declarative only.

CREATE TABLE IF NOT EXISTS conversation_analyses (
    context_id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'classified', 'failed')),
    category TEXT CHECK (category IN ('development', 'business-analysis', 'operations',
        'admin-config', 'writing-comms', 'research-learning', 'other')),
    summary TEXT,
    tags TEXT[] NOT NULL DEFAULT '{}',
    skills_used TEXT[] NOT NULL DEFAULT '{}',
    outcome TEXT CHECK (outcome IN ('achieved', 'partial', 'abandoned', 'unclear')),
    confidence REAL CHECK (confidence BETWEEN 0 AND 1),
    provider TEXT,
    model TEXT,
    ai_request_id TEXT,
    source_request_count BIGINT NOT NULL DEFAULT 0,
    source_last_at TIMESTAMPTZ,
    classified_at TIMESTAMPTZ,
    attempts INT NOT NULL DEFAULT 0,
    next_attempt TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    lease_token TEXT,
    lease_until TIMESTAMPTZ,
    last_error TEXT,
    title TEXT,
    completion SMALLINT CHECK (completion BETWEEN 0 AND 100),
    completion_rationale TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_microdollars BIGINT,
    trigger TEXT NOT NULL DEFAULT 'automatic' CHECK (trigger IN ('automatic', 'manual')),
    requested_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_conversation_analyses_pending
    ON conversation_analyses(next_attempt) WHERE status <> 'classified';

CREATE INDEX IF NOT EXISTS idx_conversation_analyses_category
    ON conversation_analyses(category, classified_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_analyses_user
    ON conversation_analyses(user_id, classified_at DESC);

CREATE INDEX IF NOT EXISTS idx_conversation_analyses_skills
    ON conversation_analyses USING gin(skills_used);

CREATE INDEX IF NOT EXISTS idx_conversation_analyses_completion
    ON conversation_analyses(completion) WHERE completion IS NOT NULL;
