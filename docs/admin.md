# Admin

There is one super-admin role. It is assigned from `SUPER_ADMIN_USER_ID` when that user exists. In development, `SUPER_ADMIN_EMAIL` can assign the role once, only if no admin exists and the email is verified. Production does not bootstrap from an email. If the user id is unset, the admin API stays closed.

The UI is `/admin`. Every admin request checks the role on the server and requires a confirmed TOTP factor. Mutations also require step-up and CSRF. Actions are disable, enable, revoke sessions, and force password reset. Each writes `ADMIN_ACTION`. There is no impersonation and no audit-log delete control.

Overview counts are cached in process for 15 seconds. User search is paginated and the sort column is an allowlist.

Services (`/admin/clients`) lists every registered OAuth client with its type, status, scopes, redirect URIs, how many users have granted consent, live access tokens, and the last authorization. The page is read-only; clients are still registered through migrations. The secret hash is never returned.
