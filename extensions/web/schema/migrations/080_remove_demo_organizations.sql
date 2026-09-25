-- Remove the seeded demo tenants. The enterprise console shows real customers
-- or it shows nothing.
--
-- Migration 025 invented three tenants (Northwind, Contoso, Initech), ten
-- members on `*.example` addresses, and 1080 synthetic `ai_requests` rows, so
-- the cross-customer view had more than one row to draw. 029 then flagged those
-- requests `synthetic = TRUE`. The cost of that was paid later: on an instance
-- carrying real users and real spend, invented tenants sit in the same tables
-- as the real ones and every list, every rollup, and every margin figure mixes
-- the two. Telling them apart became a thing a person had to know rather than
-- something the data said.
--
-- 025 and 029 are DELETED from `schema/migrations/`, not edited, so a fresh
-- install never creates these rows. Deleting the file is safe where editing it
-- is not: the runner iterates the migrations found on disk and skips the ones
-- already recorded in `extension_migrations`, so a missing file for an applied
-- version is simply never considered — no checksum comparison, no drift, no
-- `just repair-migrations`. That is the same reasoning 032 sets out for 016 and
-- 017; the difference is that those two had to keep running on fresh installs
-- and these two must not.
--
-- This file exists because deleting them does nothing for a database that
-- already applied them — every existing install, production included.
--
-- Scoped by the literal `demo-` id prefix that 025 wrote, never by email
-- domain. Real users are keyed by UUID and no real row can collide with it,
-- which is what makes this safe to run against production. `organizations` and
-- `users` cascade to their dependants (`departments`, `organization_members`,
-- `user_profile_ext`, and the rest); `ai_requests` has no foreign key to
-- `users`, so its rows are named here directly and go first.
--
-- The three organizations are also removed from
-- services/access-control/plans.yaml. Both halves are required: the bootstrap
-- loader re-asserts every organization it declares on each boot, so this
-- migration alone would delete them and the next restart would put them back.

BEGIN;

DELETE FROM ai_requests WHERE id LIKE 'demo-req-%';

DELETE FROM users WHERE id LIKE 'demo-nw-%' OR id LIKE 'demo-co-%' OR id LIKE 'demo-it-%';

DELETE FROM organizations WHERE id IN ('demo-northwind', 'demo-contoso', 'demo-initech');

COMMIT;
