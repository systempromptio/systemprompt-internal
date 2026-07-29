-- Mark the operator's own organization as the platform tenant.
--
-- The enterprise console is a cross-customer view: it lists every
-- organization, its plan, its spend, and its margin. That is the operator's
-- data, not a customer's, so holding the `admin` role inside a customer
-- organization must not open it. `organizations.is_platform` is the boundary,
-- and migration 022 already put every pre-existing user in the `house`
-- organization, which is the operator's.
--
-- Enforced as a partial unique index rather than by convention: two platform
-- tenants would mean two disjoint sets of super-admins, each invisible to the
-- other.

ALTER TABLE organizations
    ADD COLUMN IF NOT EXISTS is_platform BOOLEAN NOT NULL DEFAULT FALSE;

UPDATE organizations SET is_platform = TRUE WHERE slug = 'house';

CREATE UNIQUE INDEX IF NOT EXISTS idx_organizations_platform
    ON organizations (is_platform) WHERE is_platform;
