-- Drop the actions-per-minute metrics.
--
-- `apm`, `eapm` and `peak_concurrent` were written on every Stop event and read
-- by nothing but their own test. The measure was also unsound: `duration_minutes`
-- fell back to 1.0 whenever `ended_at` was null, so every crashed or interrupted
-- session scored as though all its work happened in one minute. Correcting that
-- to COALESCE(ended_at, last_event_at) moved every historical number, which is
-- what prompted the question of whether the metric earned its place at all. It
-- did not — nothing consumed it.
--
-- `peak_concurrent` goes with them: `update_session_apm` was its only writer, so
-- without APM it could only ever be NULL.

ALTER TABLE plugin_session_summaries
    DROP COLUMN IF EXISTS apm,
    DROP COLUMN IF EXISTS eapm,
    DROP COLUMN IF EXISTS peak_concurrent;
