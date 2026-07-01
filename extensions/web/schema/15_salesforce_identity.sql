-- Web-owned Salesforce identity side table.
--
-- Maps a local `users` row to the caller's Salesforce *Username* (the userinfo
-- `preferred_username`, e.g. `ed.aa…@agentforce.com`), captured at "Sign in with
-- Salesforce" time. The JWT-bearer grant that mints the per-user Hosted-MCP
-- bearer must use this Username as its `sub` — the login email is NOT the
-- Salesforce Username and would fail the assertion.
--
-- Keyed 1:1 to `users` and cascades on delete, mirroring `user_profile_ext` in
-- 13_web_side_tables.sql — the web extension owns its own column rather than
-- ALTERing the vendored `federated_identities` table (which stores only the
-- OIDC `sub` identity-URL, not the Username).

CREATE TABLE IF NOT EXISTS salesforce_user_identities (
    user_id     TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    sf_username TEXT NOT NULL,
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
