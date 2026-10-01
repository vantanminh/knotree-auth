# API

Errors:

```json
{ "error": { "code": "INVALID_CREDENTIALS", "message": "The email or password is incorrect.", "request_id": "req_..." } }
```

Browser session routes live under `/api/v1`. OAuth routes are unprefixed. The machine-readable description is [openapi.yaml](openapi.yaml).

Account routes require the session cookie. Mutations also require CSRF. Admin routes additionally require the super-admin role and a confirmed authenticator.

## Authorized services

`GET /api/v1/me/authorizations` lists the services (OAuth clients) the signed-in user has authorized, from `oauth_consents`. Each item has `client_id`, `name`, `first_party`, `status`, `scopes`, `granted_at`, `last_used_at` (newest token issued to that client), and `active_grants` (live refresh tokens).

`DELETE /api/v1/me/authorizations/{client_id}` removes the consent and revokes every refresh and access token the client holds for the user, which signs them out of that service. It records `OAUTH_CONSENT_REVOKED` and returns 404 when no consent exists. First-party clients skip the consent screen, so the next sign-in to one grants access again.
