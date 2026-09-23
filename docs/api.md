# API

Errors:

```json
{ "error": { "code": "INVALID_CREDENTIALS", "message": "The email or password is incorrect.", "request_id": "req_..." } }
```

Browser session routes live under `/api/v1`. OAuth routes are unprefixed. The machine-readable description is [openapi.yaml](openapi.yaml).

Account routes require the session cookie. Mutations also require CSRF. Admin routes additionally require the super-admin role and a confirmed authenticator.
