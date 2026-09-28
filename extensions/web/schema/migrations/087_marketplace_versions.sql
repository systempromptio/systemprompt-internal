-- Marketplace content hash as version identity. Twin of
-- schema/36_marketplace_versions.sql (the table and its indexes; the
-- `marketplace_version_at` function is declarative and applied before
-- migrations run).
--
-- Migration 006 dropped an unrelated legacy `marketplace_versions`; a
-- database replaying the chain from before 006 loses the declarative table
-- there, and this re-creates it.

CREATE TABLE IF NOT EXISTS marketplace_versions (
    marketplace_id TEXT NOT NULL,
    content_hash TEXT NOT NULL,
    source TEXT NOT NULL,
    source_hash TEXT,
    manifest JSONB,
    plugin_count INTEGER NOT NULL DEFAULT 0,
    skill_count INTEGER NOT NULL DEFAULT 0,
    first_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    effective_until TIMESTAMPTZ,
    origin TEXT NOT NULL DEFAULT 'manifest' CHECK (origin IN ('manifest', 'legacy_source_hash')),
    PRIMARY KEY (marketplace_id, content_hash)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_marketplace_versions_current
    ON marketplace_versions(marketplace_id) WHERE effective_until IS NULL;

CREATE INDEX IF NOT EXISTS idx_marketplace_versions_interval
    ON marketplace_versions(marketplace_id, first_seen_at, effective_until);
