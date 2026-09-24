# Email

Business code calls purpose-specific helpers. `EmailProvider` delivers them. Development uses an outbox and `GET /api/v1/dev/mailbox`, which production refuses. Production uses Resend when `EMAIL_PROVIDER=resend`.

| Template | Purpose |
| --- | --- |
| `verify-email` | Account email verification link |
| `mfa-code` | Email MFA OTP |
| `reset-password` | Password reset link |
| `new-login` | New device sign-in |
| `security-alert` | Password change, MFA change, recovery regeneration, deletion |

HTML and plain text are both stored. Logs record the template and provider id, not the body. Failed sends retry up to five times.

## DNS

Publish SPF, DKIM, and DMARC for the transactional domain before production mail. A typical sender is `Knotree Accounts <accounts@knotree.com>`. The exact records come from the chosen provider and are not hardcoded.

New-login mail includes device label, time, and IP. Location is reported as unavailable; this service does not call a geolocation vendor.
