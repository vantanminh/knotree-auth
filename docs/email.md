# Email

Business code calls purpose-specific helpers. `EmailProvider` delivers them. Development uses an outbox and `GET /api/v1/dev/mailbox`, which production refuses. Production uses Cloudflare Email Service when `EMAIL_PROVIDER=cloudflare`.

| Template | Purpose |
| --- | --- |
| `verify-email` | Account email verification link |
| `mfa-code` | Email MFA OTP |
| `reset-password` | Password reset link |
| `new-login` | New device sign-in |
| `security-alert` | Password change, MFA change, recovery regeneration, deletion |

HTML and plain text are both stored. Logs record the template and local message id, not the body. Cloudflare returns a provider message id for accepted mail. A recipient marked delivered or queued is accepted; failed sends retry up to five times.

## DNS

Onboard `knotree.com` in Cloudflare Email Service before production mail. Cloudflare configures SPF and DKIM records and an MX record on `cf-bounce`; review the generated DMARC record and any existing domain policy before accepting DNS changes. `EMAIL_FROM` is the sender address and optional `EMAIL_FROM_NAME` sets its display name.

New-login mail includes device label, time, and IP. Location is reported as unavailable; this service does not call a geolocation vendor.
