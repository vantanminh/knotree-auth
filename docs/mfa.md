# MFA

Two factors ship in this version.

**Authenticator (TOTP).** RFC 6238 via `totp-rs`: SHA-1, 6 digits, 30 seconds, one step of skew. The secret is encrypted with AES-256-GCM before it is stored and is not active until the user submits a valid code. The current step is stored as `last_used_step` so the same code cannot be replayed. Too many failures throttle the login transaction; the account is not locked forever.

**Email OTP.** Six digits from the operating-system CSPRNG, stored as a hash, single-use, bound to the login transaction and the `email_mfa` purpose.

**Recovery codes.** Eight codes, 20 characters from an unambiguous alphabet, stored as hashes. Regeneration deletes the previous batch. Use and regeneration are security events.

Disabling TOTP, changing email, revoking other sessions, deleting the account, and admin mutations require a recent step-up. Super admins cannot disable TOTP and cannot open `/admin` without a confirmed authenticator.

Passkeys and WebAuthn fit as additional `mfa_methods` rows later. They are not implemented.
