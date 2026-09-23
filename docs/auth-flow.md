# Authentication flow

1. `POST /api/v1/auth/register` with email, password, and password confirmation. The API normalizes the email, checks the password policy, hashes with Argon2id, and stores a hashed verification token.
2. The user opens `/verify-email?token=...`. The token is single-use.
3. `POST /api/v1/auth/login`. Unknown email and wrong password both return `INVALID_CREDENTIALS`. A missing user still runs a dummy Argon2 verify.
4. If TOTP or email MFA is enabled, the response is `mfa_required` and no session cookie is set. `POST /api/v1/auth/mfa/verify` completes the session.
5. The session id rotates when the user finishes step-up authentication.
6. Idle and absolute lifetimes are shorter for super admins.
7. Password reset consumes a hashed token, replaces the hash, and revokes every session.

Browser mutations require the CSRF cookie and `x-csrf-token`, and `Origin` must match `APP_BASE_URL` when the browser sends it.
