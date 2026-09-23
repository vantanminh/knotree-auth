# OAuth 2.1 and OpenID Connect

Supported grant: authorization code with PKCE S256. Refresh tokens rotate when `offline_access` was granted. Client credentials are for `client_type=service` only. Implicit and resource-owner password grants are not implemented.

Endpoints:

- `GET /oauth/authorize`
- `POST /oauth/token`
- `POST /oauth/revoke`
- `POST /oauth/introspect`
- `GET|POST /oauth/userinfo`
- `GET /.well-known/openid-configuration`
- `GET /.well-known/jwks.json`

`redirect_uri` must match a registered URI exactly. `state` is required and returned unchanged. Public clients must omit `client_secret`. Confidential and service clients authenticate with HTTP Basic or form fields. Introspection returns `active: false` for tokens issued to a different client.

First-party clients skip the consent screen. Third-party clients stop on `/oauth/consent`. The consent record is revalidated before a code is issued.

ID tokens are RS256 and include `iss`, `sub`, `aud`, `exp`, `iat`, `auth_time`, `nonce`, `amr`, and email or name claims when those scopes were granted. Access tokens stay opaque.
