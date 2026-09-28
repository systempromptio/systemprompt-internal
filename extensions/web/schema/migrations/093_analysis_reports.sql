-- On-demand AI analysis reports. Twin of the `analysis_reports` half of
-- schema/46_tool_artifacts.sql. The `tool_activity` view stays declarative
-- only: it reads core's `tool_call_ledger`, a declarative view the installer
-- creates after every migration has run, so a migration naming it fails the
-- first upgrade from a database that has never had it.

CREATE TABLE IF NOT EXISTS analysis_reports (
    id TEXT PRIMARY KEY,
    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('global', 'marketplace', 'skill', 'filter')),
    scope_id TEXT,
    scope_label TEXT,
    window_start TIMESTAMPTZ NOT NULL,
    window_end TIMESTAMPTZ NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'generated', 'failed')),
    requested_by TEXT NOT NULL,
    provider TEXT,
    model TEXT,
    ai_request_id TEXT,
    input_tokens INTEGER,
    output_tokens INTEGER,
    cost_microdollars BIGINT,
    inputs JSONB NOT NULL DEFAULT '{}'::jsonb,
    findings JSONB,
    attempts INTEGER NOT NULL DEFAULT 0,
    lease_token TEXT,
    lease_until TIMESTAMPTZ,
    next_attempt TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    last_error TEXT,
    generated_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);

CREATE INDEX IF NOT EXISTS idx_analysis_reports_created ON analysis_reports(created_at DESC);

CREATE INDEX IF NOT EXISTS idx_analysis_reports_scope ON analysis_reports(scope_kind, scope_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_analysis_reports_pending ON analysis_reports(next_attempt)
    WHERE status = 'pending';
