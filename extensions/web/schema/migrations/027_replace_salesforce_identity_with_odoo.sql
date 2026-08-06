-- Drop the Salesforce identity side table.
--
-- Salesforce is gone from this deployment: there is no "Sign in with
-- Salesforce" flow, no Hosted-MCP token accessor, and no JWT-bearer grant, so
-- nothing reads `salesforce_user_identities` any more. Odoo replaces it as the
-- CRM system of record, and the account linking it needs is a different shape
-- entirely — a login plus an encrypted per-user API key — so it gets its own
-- table (`odoo_identity`, declared in schema/15_odoo_identity.sql) rather than
-- a rename.
--
-- Nothing is migrated across. A Salesforce Username is not an Odoo login and
-- carries no credential, so every user re-links against Odoo from their
-- profile page. On a fresh install this migration is a no-op.

DROP TABLE IF EXISTS marketplace.salesforce_user_identities;
