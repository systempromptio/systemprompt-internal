-- Evaluation tables.
--
-- The gateway already records every `/v1/messages` turn in `ai_requests` and
-- the full inbound/outbound bodies in `ai_request_payloads`. These tables sit
-- on top of that spine and record what we think of those answers: judge runs,
-- per-item scores, a curated golden set, and pairwise model comparisons.
--
-- `ai_request_id` is referenced by value rather than by foreign key so an
-- eval result survives the retention sweep that eventually removes the
-- request it scored.

CREATE TABLE IF NOT EXISTS eval_runs (
    id TEXT PRIMARY KEY,
    kind TEXT NOT NULL CHECK (kind IN ('judge', 'replay', 'pairwise')),
    status TEXT NOT NULL DEFAULT 'running' CHECK (status IN ('running', 'completed', 'failed')),
    judge_provider TEXT NOT NULL,
    judge_model TEXT NOT NULL,
    filter JSONB NOT NULL DEFAULT '{}'::jsonb,
    sample_size INTEGER NOT NULL DEFAULT 0,
    scored_count INTEGER NOT NULL DEFAULT 0,
    failed_count INTEGER NOT NULL DEFAULT 0,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMPTZ,
    error_message TEXT
);

CREATE INDEX IF NOT EXISTS idx_eval_runs_created_at ON eval_runs(created_at DESC);
CREATE INDEX IF NOT EXISTS idx_eval_runs_kind ON eval_runs(kind);

CREATE TABLE IF NOT EXISTS eval_cases (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    prompt_body JSONB NOT NULL,
    source_ai_request_id TEXT,
    expectation TEXT,
    baseline_response JSONB,
    baseline_model TEXT,
    tags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    created_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_eval_cases_enabled ON eval_cases(enabled);
CREATE INDEX IF NOT EXISTS idx_eval_cases_source ON eval_cases(source_ai_request_id);

CREATE TABLE IF NOT EXISTS eval_results (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES eval_runs(id) ON DELETE CASCADE,
    ai_request_id TEXT,
    case_id TEXT REFERENCES eval_cases(id) ON DELETE SET NULL,
    user_id TEXT,
    session_id TEXT,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    overall_score INTEGER CHECK (overall_score BETWEEN 1 AND 5),
    dimension_scores JSONB NOT NULL DEFAULT '{}'::jsonb,
    verdict TEXT NOT NULL CHECK (verdict IN ('pass', 'partial', 'fail', 'skipped')),
    rationale TEXT,
    flags TEXT[] NOT NULL DEFAULT ARRAY[]::TEXT[],
    prompt_excerpt TEXT,
    response_excerpt TEXT,
    latency_ms INTEGER,
    cost_microdollars BIGINT NOT NULL DEFAULT 0,
    judge_cost_microdollars BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_eval_results_run ON eval_results(run_id);
CREATE INDEX IF NOT EXISTS idx_eval_results_model ON eval_results(model);
CREATE INDEX IF NOT EXISTS idx_eval_results_request ON eval_results(ai_request_id);
CREATE INDEX IF NOT EXISTS idx_eval_results_case ON eval_results(case_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_eval_results_run_request
    ON eval_results(run_id, ai_request_id)
    WHERE ai_request_id IS NOT NULL;

CREATE TABLE IF NOT EXISTS eval_pairs (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES eval_runs(id) ON DELETE CASCADE,
    case_id TEXT REFERENCES eval_cases(id) ON DELETE SET NULL,
    model_a TEXT NOT NULL,
    model_b TEXT NOT NULL,
    winner TEXT NOT NULL CHECK (winner IN ('a', 'b', 'tie')),
    order_swapped BOOLEAN NOT NULL DEFAULT FALSE,
    rationale TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_eval_pairs_run ON eval_pairs(run_id);
CREATE INDEX IF NOT EXISTS idx_eval_pairs_case ON eval_pairs(case_id);
