-- Cost attribution stamped at request time. Twin of
-- schema/40_request_scopes.sql: the table here, the statement trigger on
-- core's `ai_requests` declaratively (a migration may not name a
-- declarative-only trigger).
--
-- The trigger only sees requests inserted after it exists. Every request
-- already on the books is attributed here from the person's primary group
-- and project as they stand today — the best answer available, and the same
-- one the membership join gave. From this point on a request's scope is
-- fixed when it lands and moving a person never rewrites it.

CREATE TABLE IF NOT EXISTS ai_request_scopes (
    request_id TEXT PRIMARY KEY REFERENCES ai_requests(id) ON DELETE CASCADE,
    user_id TEXT NOT NULL,
    group_id TEXT REFERENCES groups(id) ON DELETE SET NULL,
    project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,
    source TEXT NOT NULL DEFAULT 'primary' CHECK (source IN ('primary', 'header')),
    resolved_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_ai_request_scopes_group
    ON ai_request_scopes(group_id);

CREATE INDEX IF NOT EXISTS idx_ai_request_scopes_project
    ON ai_request_scopes(project_id);

CREATE INDEX IF NOT EXISTS idx_ai_request_scopes_user
    ON ai_request_scopes(user_id);


INSERT INTO ai_request_scopes (request_id, user_id, group_id, project_id, source)
SELECT r.id,
       r.user_id,
       CASE WHEN r.actor_kind = 'user' THEN d.primary_group_id END,
       CASE WHEN r.actor_kind = 'user' THEN d.primary_project_id END,
       'primary'
FROM ai_requests r
LEFT JOIN user_scope_defaults d ON d.user_id = r.user_id
ON CONFLICT (request_id) DO NOTHING;
