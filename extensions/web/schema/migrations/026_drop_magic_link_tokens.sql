-- Drop the magic-link token store.
--
-- Self-service registration and magic-link sign-in are gone. There are exactly
-- two ways an account comes into existence now, and neither uses this table:
--
--   * an ordinary user arrives through Salesforce SSO, which provisions them
--     just-in-time against their organization's seat allocation;
--   * an operator is created out-of-band with `admin users create`, then enrols
--     a passkey through the setup link `admin users webauthn
--     generate-setup-token` prints.
--
-- Nothing read these rows anyway: tokens were minted and stored, but no email
-- service was ever configured to deliver them, so every row was write-only.
--
-- `webauthn_setup_tokens` is deliberately NOT touched here — it is core's
-- table, and it is what the operator passkey-enrolment flow above depends on.

DROP INDEX IF EXISTS marketplace.idx_magic_link_email_created;
DROP INDEX IF EXISTS marketplace.idx_magic_link_token_hash;
DROP TABLE IF EXISTS marketplace.magic_link_tokens;
