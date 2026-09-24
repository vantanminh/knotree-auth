# Environment

See `.env.example`. Startup fails when production is missing HTTPS, secure cookies, the JWT key, TOTP keys, or the Cloudflare account ID and Email Sending API token while `EMAIL_PROVIDER=cloudflare`. `DEV_MAILBOX=true` is rejected in production and staging.

`SUPER_ADMIN_USER_ID` is the production lock for the single admin. `SUPER_ADMIN_EMAIL` is a development convenience only.

Local URLs:

- API: `http://127.0.0.1:8080`
- Accounts UI: `http://127.0.0.1:5173`
- Dev mailbox: `GET /api/v1/dev/mailbox?email=`
