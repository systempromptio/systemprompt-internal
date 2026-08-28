INSERT INTO oauth_clients (client_id, client_secret_hash, client_name, token_endpoint_auth_method, is_active, owner_user_id)
SELECT 'marketplace-admin', NULL, 'Marketplace Admin Dashboard', 'none', true, u.id
FROM users u
WHERE 'admin' = ANY(u.roles)
ORDER BY u.created_at ASC
LIMIT 1
ON CONFLICT (client_id) DO UPDATE SET is_active = true;

-- Self-heal the owner.
--
-- The INSERT above only binds an owner the first time the client is created.
-- Migration 016 existed to guarantee that SELECT found a row on a fresh install
-- by conjuring a user ahead of `admin bootstrap` — which is where the fabricated
-- 'admin@localhost' identity came from. That guarantee is not needed: if no admin
-- exists the runtime refuses to start (RuntimeError::SystemAdminNotFound), so the
-- correct order is `admin bootstrap` (real email) -> boot -> this seed binds the
-- real admin. Seeds are idempotent and run on every boot, so this repairs an
-- install whose client points at a non-admin or at a since-demoted row.
--
-- Tie-break is oldest active admin, matching find_by_name's tie-break so the
-- seed and the runtime's resolve_system_admin agree on who "the admin" is.
UPDATE oauth_clients
   SET owner_user_id = (
        SELECT id FROM users
         WHERE 'admin' = ANY(roles) AND status = 'active'
         ORDER BY created_at ASC
         LIMIT 1
   )
 WHERE client_id = 'marketplace-admin'
   AND EXISTS (
        SELECT 1 FROM users
         WHERE 'admin' = ANY(roles) AND status = 'active'
   )
   AND (
        owner_user_id IS NULL
     OR owner_user_id NOT IN (
            SELECT id FROM users
             WHERE 'admin' = ANY(roles) AND status = 'active'
        )
   );

INSERT INTO oauth_client_grant_types (client_id, grant_type)
SELECT 'marketplace-admin', v.grant_type
FROM (VALUES ('authorization_code'), ('refresh_token')) AS v(grant_type)
WHERE EXISTS (SELECT 1 FROM oauth_clients WHERE client_id = 'marketplace-admin')
ON CONFLICT (client_id, grant_type) DO NOTHING;

INSERT INTO oauth_client_response_types (client_id, response_type)
SELECT 'marketplace-admin', 'code'
WHERE EXISTS (SELECT 1 FROM oauth_clients WHERE client_id = 'marketplace-admin')
ON CONFLICT (client_id, response_type) DO NOTHING;

INSERT INTO oauth_client_scopes (client_id, scope)
SELECT 'marketplace-admin', v.scope
FROM (VALUES ('admin'), ('user')) AS v(scope)
WHERE EXISTS (SELECT 1 FROM oauth_clients WHERE client_id = 'marketplace-admin')
ON CONFLICT (client_id, scope) DO NOTHING;

INSERT INTO oauth_client_redirect_uris (client_id, redirect_uri, is_primary)
SELECT 'marketplace-admin', v.redirect_uri, v.is_primary
FROM (VALUES
    ('/admin/login', true),
    ('http://localhost:8080/admin/login', false)
) AS v(redirect_uri, is_primary)
WHERE EXISTS (SELECT 1 FROM oauth_clients WHERE client_id = 'marketplace-admin')
ON CONFLICT (client_id, redirect_uri) DO NOTHING;
