-- @cost: rows=3644 measured=30s triggers=suspended
-- (measured on the instance this fix was first written for; the statement is
-- bounded by the mislabelled rows, which this instance has at most as many of
-- as it has Claude requests recorded since core ai/034.)
--
-- Re-derive the client of every request core migration ai/034 left mislabelled.
--
-- Claude Code ≥ 2.1.25x stamps `metadata.user_id` as a JSON object; the
-- gateway read that as OpenCode's session marker, so every Claude Code and
-- Cowork request without a bridge host token was stored as `opencode` on
-- `openai.chat`. The marker vocabulary is fixed in core; the rows are fixed
-- here, because the four per-row triggers on `ai_requests` are ours to reason
-- about. `feedback_capture` re-enqueues every request of the whole client
-- session on each row it sees — 113 ms per corrected row — and this
-- correction changes no identity, session or timing column, so no feedback
-- fact changes. The migration runner suspends every row trigger on the
-- tables a migration writes, so the explicit DISABLE/ENABLE pair this
-- migration shipped with is gone: on a database that skipped releases the
-- trigger can already be retired, and a named toggle then fails. The
-- analytics projection `reporting_capture` fed has since been retired too.
CREATE TEMPORARY TABLE ai_request_claude_markers ON COMMIT DROP AS
WITH bodies AS (
    SELECT p.ai_request_id,
           substring(
               CASE jsonb_typeof(p.request_body -> 'system')
                   WHEN 'string' THEN p.request_body ->> 'system'
                   WHEN 'array'  THEN p.request_body -> 'system' -> 0 ->> 'text'
               END
               FROM '^\s*x-anthropic-billing-header:[^\n]*cc_entrypoint=([A-Za-z0-9_-]+)') AS entrypoint
      FROM ai_request_payloads p
      JOIN ai_requests r ON r.id = p.ai_request_id
     WHERE jsonb_typeof(p.request_body) = 'object'
       AND r.client_attestation = 'native-marker'
       AND r.client_kind = 'opencode'
       AND p.request_body #>> '{metadata,user_id}' LIKE '{%'
       AND (p.request_body #>> '{metadata,user_id}')::jsonb ? 'device_id'
       AND (p.request_body #>> '{metadata,user_id}')::jsonb ? 'session_id'
)
SELECT ai_request_id,
       CASE
           WHEN entrypoint = 'cli' THEN 'claude-cli-entrypoint'
           WHEN entrypoint = 'local-agent' OR entrypoint LIKE 'claude-desktop%' THEN 'claude-desktop-entrypoint'
           ELSE 'claude-metadata-json' END AS native_marker,
       CASE
           WHEN entrypoint = 'local-agent' OR entrypoint LIKE 'claude-desktop%' THEN 'claude-desktop'
           ELSE 'claude-code' END AS client_kind
  FROM bodies;

-- Why: `metadata.user_id` is a Messages-API field; the body cannot have
-- arrived on the Chat Completions route migration ai/026 assigned it.
UPDATE ai_requests r
   SET client_kind = m.client_kind,
       wire_protocol = 'anthropic.messages'
  FROM ai_request_claude_markers m
 WHERE m.ai_request_id = r.id;

-- Rows backfilled by ai/026 never got an evidence row; the re-derived ones
-- get the marker the runtime classifier would now record.
INSERT INTO ai_request_client_evidence (ai_request_id, kind_source, native_marker)
SELECT ai_request_id, 'native-marker', native_marker
  FROM ai_request_claude_markers
ON CONFLICT (ai_request_id) DO UPDATE SET native_marker = EXCLUDED.native_marker;

DROP TABLE ai_request_claude_markers;

-- `conversation_facts.client_kind` is copied from each context's latest
-- request, so the rollup carried `opencode` too.
--
-- Why: the rollup is rebuilt by the conversation_rollup job, not here. A
-- full refresh inside the boot migration is quadratic in sessions (each
-- event is placed in its session's request windows) and cancelled a
-- 2,000-session upgrade at its statement timeout. Rewinding the job's
-- watermark makes its next tick rebuild every conversation off the boot
-- path; until then the page shows the facts it already had.
DO $$
BEGIN
    IF to_regclass('public.conversation_rollup_state') IS NOT NULL THEN
        INSERT INTO conversation_rollup_state (id, watermark)
        VALUES (TRUE, 'epoch')
        ON CONFLICT (id) DO UPDATE SET watermark = 'epoch';
    END IF;
END $$;
