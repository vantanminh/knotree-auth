# Threat model

## Assets

Account passwords, TOTP secrets, recovery codes, session cookies, refresh tokens, authorization codes, email OTPs, OAuth client secrets, signing keys, and the single super-admin role.

## Trust boundaries

| Boundary | Trust | Notes |
| --- | --- | --- |
| Browser | Untrusted | XSS on a sibling subdomain must not read the accounts cookie. |
| Cloudflare Worker | Trusted to serve UI and proxy | It does not see password hashes. It forwards cookies. |
| Authentication API | Trusted | Enforces authz, CSRF, and redirect allowlists. |
| PostgreSQL | Trusted storage, hostile if stolen | Secrets are hashed or encrypted. A dump does not reveal passwords or TOTP secrets without the app key. |
| Email provider | Trusted delivery | Bodies can contain OTPs. The provider is not an authz source. |
| Google and GitHub | External IdPs | Tokens and `nonce`/`state` are checked. Email match alone does not link accounts. |
| Knotree applications | Registered clients only | A compromised `foo.knotree.com` is not trusted merely because of the parent domain. |
| Super admin | Highly trusted human | Shorter sessions, TOTP required, step-up on mutations, audit log. |

## Chosen attacks and controls

- **Credential stuffing:** Argon2id, generic login errors, per-email and per-IP failure windows in `auth_attempts`.
- **Session theft via subdomain:** host-only `__Host-` cookies in production, no `Domain=.knotree.com`.
- **Sibling CSRF:** double-submit CSRF token plus `Origin` must equal `APP_BASE_URL`. `SameSite=Lax` is not sufficient for same-site sibling POSTs.
- **Open redirect:** OAuth `redirect_uri` is an exact match against the client registry. In-app `return_to` must be a relative path.
- **Authorization code replay:** single use; reuse revokes the session and its tokens.
- **Refresh reuse:** family revocation.
- **PKCE downgrade:** `S256` is required. Public clients must not send a client secret.
- **Account enumeration:** login and password reset responses are generic. Registration returns 409 when the email exists, which is an accepted product tradeoff.
- **Admin bypass:** `is_admin` is loaded from `role_assignments` and `SUPER_ADMIN_USER_ID` on each session load. Profile updates use `deny_unknown_fields`.
- **TOTP database theft:** AES-256-GCM with an application key that is not in the database.
- **OTP logging:** templates are logged by name. Bodies and codes are not logged. Admin email logs omit bodies.

## Residual risk

Approximate location is not collected; new-login mail says location is unavailable. A stolen TOTP encryption key plus a database dump exposes TOTP secrets. Key rotation is supported by versioned keys, but re-encryption of existing rows is an operational follow-up. In-process stats cache is per replica and can be 15 seconds stale.
