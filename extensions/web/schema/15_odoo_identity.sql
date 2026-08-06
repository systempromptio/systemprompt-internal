-- Web-owned Odoo identity side table.
--
-- Maps a local `users` row to the Odoo account that user's tool calls execute
-- as. Odoo is the system of record for CRM, and every JSON-RPC call the odoo
-- MCP server makes is issued with *this* user's login and API key — never a
-- shared service account — so Odoo's own access rules and audit trail apply
-- unchanged to work an agent does on the user's behalf.
--
-- `odoo_uid` is the integer Odoo returns from `common.authenticate`; it is
-- cached here because `execute_kw` takes the uid, not the login, and
-- re-authenticating on every tool call would double the round trips.
--
-- The API key is stored encrypted (ChaCha20-Poly1305 under the deployment
-- master key, nonce prefixed, hex-encoded) by
-- `repositories::users::odoo_identity`, which is the only code that reads it.
--
-- Keyed 1:1 to `users` and cascades on delete, mirroring `user_profile_ext` in
-- 13_web_side_tables.sql — the web extension owns its own table rather than
-- ALTERing a vendored one.

CREATE TABLE IF NOT EXISTS odoo_identity (
    user_id              TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    odoo_login           TEXT NOT NULL,
    odoo_uid             INTEGER NOT NULL,
    odoo_api_key_encrypted TEXT NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_odoo_identity_login ON odoo_identity(odoo_login);
