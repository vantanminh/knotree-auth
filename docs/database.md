# Database

PostgreSQL 16. Migrations live in `backend/migrations` and run on API startup with SQLx.

Core tables:

- `users`, `user_emails`, `identities`, `password_credentials` separate the person from each login method.
- `sessions` store hashed browser session tokens, device label, IP, MFA and step-up timestamps.
- `refresh_tokens` store hashed refresh tokens, `family_id`, `used_at`, and `replaced_by`.
- `mfa_methods`, `totp_credentials`, `recovery_codes`, `email_challenges` cover TOTP, recovery, verification, reset, email MFA, and email change. Challenge `purpose` keeps those credentials distinct.
- `login_transactions` hold the short-lived MFA step after a correct password.
- `oauth_clients`, `oauth_authorization_codes`, `oauth_consents`, `oauth_consent_requests`, `oauth_access_tokens`, `service_accounts` are the client registry and grants.
- `role_assignments` holds `super_admin`. The API also locks that role to `SUPER_ADMIN_USER_ID` when set.
- `security_events` is append-only from the application. There is no delete API.
- `auth_attempts` backs rate limits.
- `email_messages` records delivery status. Admin reads omit the body.
- `social_transactions` store the encrypted PKCE verifier for Google and GitHub.

Uniqueness on normalized email and on `(provider, subject)` closes registration and linking races. Authorization-code and refresh-token consumption use `FOR UPDATE` and a conditional update.
