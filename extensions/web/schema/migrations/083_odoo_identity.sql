-- Twin of schema/15_odoo_identity.sql, restored.
--
-- The declaration was deleted in 2f9efe57 while the odoo MCP server still
-- read and wrote the table, so a database installed after that commit has no
-- `odoo_identity` and every Odoo tool call fails to find a credential. A
-- database installed before it keeps its table and rows; this is a no-op
-- there.

CREATE TABLE IF NOT EXISTS odoo_identity (
    user_id              TEXT PRIMARY KEY REFERENCES users(id) ON DELETE CASCADE,
    odoo_login           TEXT NOT NULL,
    odoo_uid             INTEGER NOT NULL,
    odoo_api_key_encrypted TEXT NOT NULL,
    created_at           TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at           TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_odoo_identity_login ON odoo_identity(odoo_login);
