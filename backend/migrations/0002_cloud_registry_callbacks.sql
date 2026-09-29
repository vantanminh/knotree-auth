-- Server-side callbacks use host-only application sessions and S256 PKCE.
-- Keep existing redirects for clients still using their earlier UI callback.
UPDATE oauth_clients
SET redirect_uris = ARRAY(
    SELECT DISTINCT uri FROM unnest(redirect_uris || ARRAY['https://cloud.knotree.com/api/v1/auth/sso/callback']) AS uri
)
WHERE id = 'knotree-cloud';

UPDATE oauth_clients
SET redirect_uris = ARRAY(
    SELECT DISTINCT uri FROM unnest(redirect_uris || ARRAY['https://registry.knotree.com/api/v1/auth/sso/callback']) AS uri
)
WHERE id = 'knotree-registry';
