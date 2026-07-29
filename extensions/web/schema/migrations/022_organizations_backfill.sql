-- Seed the house organization and pull every pre-existing user, department,
-- and the built-in plans onto it.
--
-- Pooled tenancy only works if org_id is never NULL: a NULL org is a row no
-- organization rule can match and no seat count includes, which reads as
-- "invisible to billing and to authz" rather than as an error. This migration
-- is what makes that invariant true on installs that predate organizations.

INSERT INTO plans (id, name, description, seat_limit, monthly_cost_cap_microdollars)
VALUES ('house', 'House', 'Astound-internal. Unlimited seats, no spend cap.', NULL, NULL)
ON CONFLICT (id) DO NOTHING;

INSERT INTO organizations (id, slug, name, plan_id)
VALUES ('house', 'house', 'Astound Digital', 'house')
ON CONFLICT (id) DO NOTHING;

INSERT INTO organization_members (user_id, org_id, org_role)
SELECT u.id, 'house', CASE WHEN 'admin' = ANY(u.roles) THEN 'owner' ELSE 'member' END
FROM users u
WHERE NOT EXISTS (SELECT 1 FROM organization_members m WHERE m.user_id = u.id)
ON CONFLICT (user_id) DO NOTHING;

ALTER TABLE departments ADD COLUMN IF NOT EXISTS org_id TEXT;

UPDATE departments SET org_id = 'house' WHERE org_id IS NULL;

ALTER TABLE departments ALTER COLUMN org_id SET NOT NULL;

ALTER TABLE departments DROP CONSTRAINT IF EXISTS departments_org_fk;
ALTER TABLE departments ADD CONSTRAINT departments_org_fk
    FOREIGN KEY (org_id) REFERENCES organizations(id) ON DELETE CASCADE;

CREATE INDEX IF NOT EXISTS idx_departments_org ON departments(org_id);

-- Department names are unique per organization, not globally: two customers
-- may both run a "Sales", and before this swap the second one to be created
-- would have collided with the first.
ALTER TABLE departments DROP CONSTRAINT IF EXISTS departments_name_key;

CREATE UNIQUE INDEX IF NOT EXISTS idx_departments_org_name ON departments(org_id, name);
