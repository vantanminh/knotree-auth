# Email

Business code calls purpose-specific helpers. `EmailProvider` delivers them. Development uses an outbox and `GET /api/v1/dev/mailbox`, which production refuses. Production uses Cloudflare Email Service when `EMAIL_PROVIDER=cloudflare`.

| Template | Purpose |
| --- | --- |
| `verify-email` | Account email verification link |
| `mfa-code` | Email MFA OTP |
| `reset-password` | Password reset link |
| `new-login` | New device sign-in |
| `security-alert` | Password change, MFA change, recovery regeneration, deletion |

HTML and plain text are both stored. Logs record the template, local message id, and Cloudflare's error code. They do not record the body or the provider's raw error text. Recipients are sent as a list, and a display name is sent as `{address, name}`. A recipient marked delivered or queued is accepted. HTTP 429 and 5xx retries run up to five times within a day. HTTP 400, 401, 403, and 404, along with bounces and suppressions, are not retried.

## DNS

Onboard `knotree.com` in Cloudflare Email Service before production mail. Cloudflare configures SPF and DKIM records and an MX record on `cf-bounce`; review the generated DMARC record and any existing domain policy before accepting DNS changes. `EMAIL_FROM` is the sender address and optional `EMAIL_FROM_NAME` sets its display name.

New-login mail includes device label, time, and IP. Location is reported as unavailable; this service does not call a geolocation vendor.
