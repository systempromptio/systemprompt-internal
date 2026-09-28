-- 2026-05-07 catalog refactor: marketplaces moved to YAML-defined services.
-- These tables no longer exist in the declarative schema; drop leftovers
-- from pre-refactor deployments.
--
-- @supersedes-checksum: 82ee18e928df0a8c
--
-- `marketplace_versions` is a declarative table again (schema/36, migration
-- 087), so only its legacy shape is dropped, and in a DO block: a migration
-- made only of `DROP … IF EXISTS` is a retirement, which core executes on a
-- fresh install after the structural DDL, and would drop the new table.

DROP TABLE IF EXISTS marketplace_changelog CASCADE;
DROP TABLE IF EXISTS org_marketplace_plugins CASCADE;
DROP TABLE IF EXISTS org_marketplaces CASCADE;
DROP TABLE IF EXISTS plugin_ratings CASCADE;
DROP TABLE IF EXISTS plugin_visibility_rules CASCADE;
DROP TABLE IF EXISTS plugin_installations CASCADE;
DROP TABLE IF EXISTS plugin_installation_history CASCADE;
DROP TABLE IF EXISTS org_marketplace_sync_logs CASCADE;

DO $$
BEGIN
    IF to_regclass('marketplace_versions') IS NOT NULL
       AND NOT EXISTS (
           SELECT 1 FROM information_schema.columns
           WHERE table_schema = current_schema()
             AND table_name = 'marketplace_versions'
             AND column_name = 'content_hash'
       ) THEN
        DROP TABLE marketplace_versions CASCADE;
    END IF;
END $$;
