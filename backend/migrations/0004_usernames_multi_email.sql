-- A Knotree account has a unique username and up to three emails (one primary).
-- Sign-in accepts the username or any verified email, so the password belongs
-- to the account rather than to one address.

ALTER TABLE users ADD COLUMN username TEXT;
ALTER TABLE users ADD COLUMN username_changed_at TIMESTAMPTZ;

-- Backfill from the primary email's local part. A taken, reserved or too-short
-- stem gets a random suffix from the user id.
WITH base AS (
    SELECT
        u.id,
        u.created_at,
        left(
            trim(both '-' from regexp_replace(lower(split_part(coalesce(e.email, ''), '@', 1)), '[^a-z0-9]+', '-', 'g')),
            30
        ) AS stem
    FROM users u
    LEFT JOIN user_emails e ON e.user_id = u.id AND e.is_primary
),
cleaned AS (
    SELECT
        id,
        created_at,
        CASE
            WHEN char_length(trim(both '-' from stem)) >= 3
                AND trim(both '-' from stem) NOT IN (
                    'admin', 'administrator', 'root', 'system', 'support', 'help', 'security',
                    'knotree', 'accounts', 'account', 'api', 'www', 'mail', 'oauth', 'auth',
                    'login', 'logout', 'signin', 'signup', 'sign-in', 'sign-up', 'register',
                    'settings', 'me', 'user', 'users', 'null', 'undefined', 'registry', 'cloud',
                    'deleted'
                )
            THEN trim(both '-' from stem)
            ELSE 'user'
        END AS stem
    FROM base
),
numbered AS (
    SELECT id, stem, row_number() OVER (PARTITION BY stem ORDER BY created_at, id) AS n
    FROM cleaned
)
UPDATE users u
SET username = CASE
    WHEN numbered.n = 1 AND numbered.stem <> 'user' THEN numbered.stem
    ELSE numbered.stem || '-' || right(replace(u.id::text, '-', ''), 8)
END
FROM numbered
WHERE numbered.id = u.id;

ALTER TABLE users ALTER COLUMN username SET NOT NULL;
ALTER TABLE users ADD CONSTRAINT users_username_format
    CHECK (username ~ '^[a-z0-9](?:[a-z0-9-]{1,37}[a-z0-9])$');
CREATE UNIQUE INDEX users_username_unique ON users (username);

-- A released username stays reserved for its previous owner for a while so a
-- rename cannot be used to impersonate them right away.
CREATE TABLE username_holds (
    username TEXT PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    released_until TIMESTAMPTZ NOT NULL
);

-- The password identity is keyed by the account, not by an email.
UPDATE identities SET provider_subject = user_id::text WHERE provider = 'password';

ALTER TABLE email_challenges DROP CONSTRAINT IF EXISTS email_challenges_purpose_check;
ALTER TABLE email_challenges ADD CONSTRAINT email_challenges_purpose_check
    CHECK (purpose IN ('email_verify', 'password_reset', 'email_mfa', 'email_change', 'email_add'));

CREATE INDEX user_emails_verified_lookup ON user_emails (email) WHERE verified_at IS NOT NULL;
