# Environment

See `.env.example`. Startup fails when production is missing HTTPS, secure cookies, the JWT key, TOTP keys, or the Cloudflare account ID and Email Sending API token while `EMAIL_PROVIDER=cloudflare`. `DEV_MAILBOX=true` is rejected in production and staging.

`SUPER_ADMIN_USER_ID` is the production lock for the single admin and takes priority.

`SUPER_ADMIN_EMAIL` + `SUPER_ADMIN_PASSWORD` seed the super admin in any environment: at startup the account (username `SUPER_ADMIN_USERNAME`, default `admin`) is created with a verified email if it does not exist, and becomes the only super admin. The password must satisfy the normal password policy or startup fails. An existing account's password is never overwritten, so a password changed in the app survives restarts. Keep `SUPER_ADMIN_PASSWORD` in a secret. Without a password, `SUPER_ADMIN_EMAIL` only promotes an existing verified account and is ignored in production.

`CLIENT_LOGO_DIR` (default `./data/client-logos`) is where service logos uploaded by the super admin are stored. It is created at startup; in Kubernetes mount a persistent volume there.

Local URLs:

- API: `http://127.0.0.1:8080`
- Accounts UI: `http://127.0.0.1:5173`
- Dev mailbox: `GET /api/v1/dev/mailbox?email=`
