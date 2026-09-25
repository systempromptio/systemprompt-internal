-- Session registry: make an agent session addressable, and its work locatable.
--
-- `plugin_session_summaries` is the Claude Code session row, but it records
-- only what a session *did* (counters, tokens, AI summary) and nothing about
-- where or when it is doing it. Two consequences:
--
--   1. Nothing can address a session. `/admin/entities/sessions` renders the
--      viewer's own JWT and queries nothing, because there is nothing to show.
--   2. No report in the product attributes cost to a repository. Every hook
--      event already carries `cwd` and we store it on `plugin_usage_events`,
--      but it is read in exactly one place -- to interpolate a repo name into
--      an AI summary sentence -- and then discarded.
--
-- Every column below except `git_branch` is derived from data already arriving
-- on the hook plane; `git_branch` is supplied by a SessionStart hook script.
--
-- `handle` is the address: `<workspace>` or `<workspace>:<branch>`, with a
-- `#N` suffix when one user holds several live sessions on the same base. The
-- uniqueness index is PARTIAL on live sessions, so a handle is reclaimed when
-- the session ends rather than being consumed forever.
--
-- Cost is microdollars (BIGINT) to match `ai_requests.cost_microdollars`; a
-- float dollar column here would silently disagree with every existing rollup.

ALTER TABLE plugin_session_summaries
    ADD COLUMN IF NOT EXISTS cwd TEXT,
    ADD COLUMN IF NOT EXISTS workspace TEXT,
    ADD COLUMN IF NOT EXISTS git_branch TEXT,
    ADD COLUMN IF NOT EXISTS handle TEXT,
    ADD COLUMN IF NOT EXISTS last_event_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS current_activity TEXT,
    ADD COLUMN IF NOT EXISTS live_cost_microdollars BIGINT,
    ADD COLUMN IF NOT EXISTS context_pct SMALLINT;

-- Backfill liveness for existing rows so `last_event_at` is never null for a
-- session that already has activity. `updated_at` is the closest existing
-- proxy: it is touched on every counter increment.
UPDATE plugin_session_summaries
    SET last_event_at = COALESCE(ended_at, updated_at)
    WHERE last_event_at IS NULL;

-- One live handle per user. Partial, so ended sessions release theirs.
CREATE UNIQUE INDEX IF NOT EXISTS idx_session_summary_handle
    ON plugin_session_summaries (user_id, handle)
    WHERE handle IS NOT NULL AND ended_at IS NULL;

-- Per-repository attribution: the first join key of its kind in the product.
CREATE INDEX IF NOT EXISTS idx_session_summary_workspace
    ON plugin_session_summaries (workspace)
    WHERE workspace IS NOT NULL;

-- Liveness scans for the live-sessions board.
CREATE INDEX IF NOT EXISTS idx_session_summary_last_event
    ON plugin_session_summaries (last_event_at DESC);
