-- Give the bootstrap admin a real identity, and stop using an email as a key.
--
-- Migrations 016 and 017 invented one. 016 INSERTed a user row so the
-- `marketplace-admin` OAuth client had an `owner_user_id` on a fresh install,
-- filling `email` with 'admin@localhost', `full_name`/`display_name` with
-- 'Platform Admin', and asserting `email_verified = true` about an address
-- nobody owns. 017 then rewrote the email to 'admin@localhost.dev' for a single
-- reason: core's `resolve_local_user_email()` returned that exact literal for
-- local-trial profiles, so the two had to agree for `find_by_email` to hit.
--
-- That made `users.email` a rendezvous key between two pieces of code rather
-- than an identity — and it surfaced: the bridge device-link consent page
-- renders the account email as the identity a durable personal access token is
-- about to be minted for, so the operator was asked to approve 'admin@localhost.dev'.
-- Recognising the account is the only control that flow has; a fabricated
-- address removes it.
--
-- Core no longer keys on the literal (it resolves the admin by
-- `system_admin.username` via `find_by_name`, as the runtime already did), so
-- the email is free to be real.
--
-- Renamed IN PLACE, not merged. This row's `id` anchors `odoo_identity` (with
-- the encrypted Odoo API key), a bridge PAT in `user_api_keys`, its
-- `user_sessions`, `user_contexts`, and `oauth_clients.owner_user_id` — 20+ FKs,
-- several of them PK-on-user_id side tables where repointing can collide.
-- Rewriting three string columns touches none of them.
--
-- `name` stays 'admin'. It is the configured `system_admin.username`, the
-- `owner:` of every scheduler job, and the marketplace-admin client owner — a
-- service principal, and the one field here that was never a claim about a person.
--
-- 016 and 017 are deliberately left untouched, comments included:
-- `extension_migrations.checksum` is recorded per file, so editing an applied
-- migration drifts every existing install into needing `just repair-migrations`.
-- This file is where their history is written down instead.

BEGIN;

DO $$
DECLARE
    -- The address is real and already proven against this install: it is the
    -- Odoo login bound to this very row (odoo_identity.odoo_login, uid 8).
    real_email    CONSTANT text := 'ed@systemprompt.io';
    real_name     CONSTANT text := 'Edward Burton';
    synthetic     CONSTANT text := 'admin@localhost.dev';
    target_id     text;
    holder_id     text;
BEGIN
    SELECT id INTO target_id FROM users WHERE email = synthetic;

    IF target_id IS NULL THEN
        -- Fresh install (016's row never existed or was already renamed).
        RETURN;
    END IF;

    -- `users.email` is UNIQUE. If a *different* row already holds the real
    -- address then this is a merge, not a rename: the two rows' side tables
    -- (odoo_identity, organization members — anything keyed PRIMARY KEY
    -- (user_id)) can collide, and picking a survivor is a judgement call about
    -- which sessions and API keys stay valid. Refuse loudly and let an operator
    -- run it by hand rather than corrupt either row.
    SELECT id INTO holder_id FROM users WHERE email = real_email;
    IF holder_id IS NOT NULL AND holder_id <> target_id THEN
        RAISE EXCEPTION
            'Cannot rename bootstrap admin %: % is already held by user %. '
            'This needs a hand-run merge, not a migration.',
            target_id, real_email, holder_id;
    END IF;

    UPDATE users
       SET email          = real_email,
           -- Nothing on this install ever verified this address. 016 asserted
           -- `true` about an address that did not exist; carrying that forward
           -- onto a real one would be the same lie about a live mailbox.
           email_verified = false,
           -- 'Platform Admin' is a role wearing a person's field. Same defect
           -- one level down from the email, so it goes with it.
           full_name      = real_name,
           display_name   = real_name,
           updated_at     = CURRENT_TIMESTAMP
     WHERE id = target_id;
END $$;

-- The rename must not have cost the marketplace-admin client its owner: the id
-- is unchanged, so this asserts the invariant rather than repairing anything.
--
-- Only meaningful once the client exists. Seeds run after migrations, so on a
-- fresh install there is no row yet and there is nothing to assert about.
DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM oauth_clients WHERE client_id = 'marketplace-admin')
       AND NOT EXISTS (
        SELECT 1
          FROM oauth_clients c
          JOIN users u ON u.id = c.owner_user_id
         WHERE c.client_id = 'marketplace-admin'
           AND 'admin' = ANY(u.roles)
    ) THEN
        RAISE EXCEPTION 'marketplace-admin has no admin owner after 032';
    END IF;
END $$;

COMMIT;
