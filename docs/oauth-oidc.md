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

First-party clients skip the consent screen. Third-party clients stop on `/oauth/consent`. That path, and `/oauth/error`, are Accounts UI screens. The edge serves them from the app. It forwards only `/oauth/authorize`, `/oauth/token`, `/oauth/revoke`, `/oauth/introspect`, and `/oauth/userinfo` to the API. The consent record is revalidated before a code is issued.

ID tokens are RS256 and include `iss`, `sub`, `aud`, `exp`, `iat`, `auth_time`, `nonce`, `amr`, and email or name claims when those scopes were granted. The `profile` scope also adds `preferred_username`, the account's username. `email` is always the primary email. Usernames and emails can change, so relying services must key users on `sub` only and refresh the other claims at every sign-in. Access tokens stay opaque.
