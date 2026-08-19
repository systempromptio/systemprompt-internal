-- The house plan and platform organization are target state, not history:
-- core 0.32 stamps fresh installs (migrations are recorded, not executed), so
-- the seeds in migrations/022_organizations_backfill.sql never run there and
-- must exist as boot seeds too. Names are the post-028 rebrand; ON CONFLICT
-- DO NOTHING keeps an operator's rename.
INSERT INTO plans (id, name, description, seat_limit, monthly_cost_cap_microdollars)
VALUES ('house', 'House', 'Internal. Unlimited seats, no spend cap.', NULL, NULL)
ON CONFLICT (id) DO NOTHING;

-- is_platform is what gates the cross-customer console (migration 024 flips
-- it on upgraded installs); the house org is the platform tenant.
INSERT INTO organizations (id, slug, name, plan_id, is_platform)
VALUES ('house', 'house', 'Systemprompt Internal', 'house', TRUE)
ON CONFLICT (id) DO NOTHING;
