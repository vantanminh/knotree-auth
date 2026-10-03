# Integrate a Knotree application

Knotree Accounts is the only credential store. A new service registers as an OAuth client and keeps its own local session after the callback.

1. Insert a row in `oauth_clients` with a stable `client_id`, display name, exact `redirect_uris`, allowed scopes, and `client_type` `public` or `confidential`. First-party clients may set `first_party` so the consent screen is skipped. Do not use wildcard redirect URIs.
2. Put the `client_id` in the service configuration. Confidential clients also store a secret whose SHA-256 hash is `secret_hash`.
3. When the user chooses Sign in, generate a PKCE verifier and `S256` challenge, plus `state` and `nonce`. Store the verifier and state in the service’s own session.
4. Redirect the browser to:

```text
https://accounts.knotree.com/oauth/authorize
  ?client_id=knotree-study
  &redirect_uri=https://study.knotree.com/auth/callback
  &response_type=code
  &scope=openid%20profile%20email
  &state=...
  &code_challenge=...
  &code_challenge_method=S256
  &nonce=...
```

   For a Sign up button, send the same request with `&screen_hint=signup`.
   A visitor without a session lands on account creation instead of sign-in.
   The request is kept through email verification, so the new user returns
   to the service after confirming their address.

5. On the callback, reject a missing or mismatched `state`.
6. `POST https://accounts.knotree.com/oauth/token` with `grant_type=authorization_code`, the code, the same `redirect_uri`, `client_id`, and `code_verifier`.
7. Verify the ID token against `/.well-known/jwks.json`: RS256, `iss` is `https://accounts.knotree.com`, `aud` contains the client id, `exp` is in the future, and `nonce` matches.
8. Create a local application session from `sub`. Do not copy the accounts session cookie into the service.

`examples/knotree-study` is a small public-client callback server for local development. Seeded clients already include study, registry, cloud, dashboard, drive, and app for their production callback URLs. Development startup adds `http://127.0.0.1:4174/auth/callback` on `knotree-study`.
