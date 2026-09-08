-- The catch-all "Default" department is target state, not history: core 0.32
-- stamps fresh installs (migrations are recorded, not executed), so the seed
-- in migrations/009_departments_default.sql never runs there and must exist
-- as a boot seed too. WHERE NOT EXISTS rather than ON CONFLICT because
-- upgraded databases swap UNIQUE(name) for UNIQUE(org_id, name) in 022, so
-- the conflict target is left unnamed (the seed linter requires ON CONFLICT
-- on every INSERT; the NOT EXISTS guard does the real idempotency work).
INSERT INTO departments (org_id, name, description)
SELECT 'house', 'Default', 'Default department; contains every user without an explicit assignment.'
WHERE NOT EXISTS (SELECT 1 FROM departments WHERE org_id = 'house' AND name = 'Default')
ON CONFLICT DO NOTHING;
