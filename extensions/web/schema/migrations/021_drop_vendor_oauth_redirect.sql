-- Remove the vendor demo-host redirect URI that earlier seeds installed.
-- Deployments derive their external redirect from EXTERNAL_URL at runtime;
-- a literal foreign host must never ship in the template.
DELETE FROM oauth_client_redirect_uris
WHERE client_id = 'marketplace-admin'
  AND redirect_uri = 'https://f7ae798f9c2a.systemprompt.io/admin/login';
