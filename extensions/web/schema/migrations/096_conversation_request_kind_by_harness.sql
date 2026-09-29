-- The conversation classifier, rebuilt by harness binding. Declarative twin:
-- schema/27_conversation_requests.sql.
--
-- `conversation_request_kind` gained a fourth argument (`harness_bound`), the
-- `conversation_requests` view gained `client_kind`/`client_attestation` in
-- the middle of its column list and drops job-made requests, and
-- `conversation_metrics_for` returns the two client columns. None of these
-- can be replaced in place: a view's columns cannot be reordered and a
-- function's result type cannot change under CREATE OR REPLACE. So the old
-- objects are dropped here and the dependent phase, which runs after every
-- migration, re-creates all of them from the declarative file.
--
-- CASCADE takes the objects built on them: the `conversation_requests` view
-- (on the three-argument classifier) and the `conversation_rollups` view (on
-- `conversation_metrics_for`). Both are declarative and derived; nothing is
-- lost. The SQL functions that read them (`conversation_opening_prompt`,
-- `refresh_conversation_facts`) hold no dependency and are untouched. A fresh
-- database stamps this migration without running it.

DROP FUNCTION IF EXISTS conversation_request_kind(text, boolean, bigint) CASCADE;
DROP FUNCTION IF EXISTS conversation_metrics_for(text[]) CASCADE;
