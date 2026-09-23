-- Knotree Accounts initial schema.
-- Tokens, OTPs, recovery codes, and client secrets are stored as hashes.
-- TOTP secrets are stored as AES-256-GCM ciphertext, not plaintext.

CREATE TABLE users (
    id UUID PRIMARY KEY,
    display_name TEXT,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled', 'pending_deletion')),
    must_reset_password BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL,
    last_login_at TIMESTAMPTZ,
    password_changed_at TIMESTAMPTZ,
    deletion_requested_at TIMESTAMPTZ,
    CONSTRAINT users_display_name_len CHECK (display_name IS NULL OR char_length(display_name) BETWEEN 1 AND 80)
);

CREATE TABLE user_emails (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT TRUE,
    verified_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT user_emails_email_unique UNIQUE (email),
    CONSTRAINT user_emails_email_len CHECK (char_length(email) BETWEEN 3 AND 254)
);

CREATE UNIQUE INDEX user_emails_one_primary ON user_emails (user_id) WHERE is_primary;
CREATE INDEX user_emails_user ON user_emails (user_id);

CREATE TABLE identities (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    provider TEXT NOT NULL CHECK (provider IN ('password', 'google', 'github')),
    provider_subject TEXT NOT NULL,
    email TEXT,
    email_verified BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL,
    last_used_at TIMESTAMPTZ,
    CONSTRAINT identities_provider_subject UNIQUE (provider, provider_subject)
);

CREATE INDEX identities_user ON identities (user_id);

CREATE TABLE password_credentials (
    identity_id UUID PRIMARY KEY REFERENCES identities (id) ON DELETE CASCADE,
    password_hash TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL
);

CREATE TABLE sessions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL,
    last_active_at TIMESTAMPTZ NOT NULL,
    authenticated_at TIMESTAMPTZ NOT NULL,
    elevated_at TIMESTAMPTZ,
    expires_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    revoke_reason TEXT,
    ip INET,
    user_agent TEXT,
    device_label TEXT NOT NULL,
    auth_methods TEXT[] NOT NULL,
    mfa_satisfied BOOLEAN NOT NULL DEFAULT FALSE
);

CREATE INDEX sessions_user_active ON sessions (user_id, last_active_at DESC) WHERE revoked_at IS NULL;

CREATE TABLE refresh_tokens (
    id UUID PRIMARY KEY,
    family_id UUID NOT NULL,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    session_id UUID REFERENCES sessions (id) ON DELETE SET NULL,
    client_id TEXT NOT NULL,
    token_hash BYTEA NOT NULL UNIQUE,
    scopes TEXT[] NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    replaced_by UUID
);

CREATE INDEX refresh_tokens_family ON refresh_tokens (family_id);
CREATE INDEX refresh_tokens_session ON refresh_tokens (session_id);

CREATE TABLE mfa_methods (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    method TEXT NOT NULL CHECK (method IN ('totp', 'email')),
    created_at TIMESTAMPTZ NOT NULL,
    enabled_at TIMESTAMPTZ,
    disabled_at TIMESTAMPTZ,
    CONSTRAINT mfa_methods_user_method UNIQUE (user_id, method)
);

CREATE TABLE totp_credentials (
    mfa_method_id UUID PRIMARY KEY REFERENCES mfa_methods (id) ON DELETE CASCADE,
    secret_ciphertext BYTEA NOT NULL,
    secret_nonce BYTEA NOT NULL,
    key_version SMALLINT NOT NULL,
    algorithm TEXT NOT NULL DEFAULT 'SHA1',
    digits SMALLINT NOT NULL DEFAULT 6,
    period_seconds SMALLINT NOT NULL DEFAULT 30,
    confirmed_at TIMESTAMPTZ,
    last_used_step BIGINT
);

CREATE TABLE recovery_codes (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    code_hash BYTEA NOT NULL,
    batch_id UUID NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ
);

CREATE INDEX recovery_codes_user_unused ON recovery_codes (user_id) WHERE used_at IS NULL;

CREATE TABLE email_challenges (
    id UUID PRIMARY KEY,
    user_id UUID REFERENCES users (id) ON DELETE CASCADE,
    email TEXT NOT NULL,
    purpose TEXT NOT NULL CHECK (purpose IN ('email_verify', 'password_reset', 'email_mfa', 'email_change')),
    code_hash BYTEA NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    max_attempts INTEGER NOT NULL DEFAULT 5,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX email_challenges_lookup ON email_challenges (purpose, code_hash) WHERE consumed_at IS NULL;
CREATE INDEX email_challenges_user ON email_challenges (user_id, purpose, created_at DESC);

CREATE TABLE login_transactions (
    id UUID PRIMARY KEY,
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    token_hash BYTEA NOT NULL UNIQUE,
    created_at TIMESTAMPTZ NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    ip INET,
    user_agent TEXT,
    attempt_count INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE oauth_clients (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    client_type TEXT NOT NULL CHECK (client_type IN ('public', 'confidential', 'service')),
    secret_hash BYTEA,
    redirect_uris TEXT[] NOT NULL DEFAULT '{}',
    allowed_scopes TEXT[] NOT NULL,
    first_party BOOLEAN NOT NULL DEFAULT FALSE,
    require_pkce BOOLEAN NOT NULL DEFAULT TRUE,
    status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'disabled')),
    created_at TIMESTAMPTZ NOT NULL,
    CONSTRAINT oauth_clients_id_len CHECK (char_length(id) BETWEEN 1 AND 128)
);

CREATE TABLE oauth_authorization_codes (
    id UUID PRIMARY KEY,
    code_hash BYTEA NOT NULL UNIQUE,
    client_id TEXT NOT NULL REFERENCES oauth_clients (id),
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    session_id UUID NOT NULL REFERENCES sessions (id) ON DELETE CASCADE,
    redirect_uri TEXT NOT NULL,
    scopes TEXT[] NOT NULL,
    nonce TEXT,
    code_challenge TEXT NOT NULL,
    code_challenge_method TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    used_at TIMESTAMPTZ
);

CREATE TABLE oauth_consents (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    client_id TEXT NOT NULL REFERENCES oauth_clients (id) ON DELETE CASCADE,
    scopes TEXT[] NOT NULL,
    granted_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, client_id)
);

CREATE TABLE service_accounts (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    client_id TEXT NOT NULL UNIQUE REFERENCES oauth_clients (id),
    created_at TIMESTAMPTZ NOT NULL,
    disabled_at TIMESTAMPTZ
);

CREATE TABLE oauth_access_tokens (
    id UUID PRIMARY KEY,
    token_hash BYTEA NOT NULL UNIQUE,
    user_id UUID REFERENCES users (id) ON DELETE CASCADE,
    service_account_id UUID REFERENCES service_accounts (id) ON DELETE CASCADE,
    client_id TEXT NOT NULL REFERENCES oauth_clients (id),
    session_id UUID REFERENCES sessions (id) ON DELETE SET NULL,
    scopes TEXT[] NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    revoked_at TIMESTAMPTZ,
    CONSTRAINT access_token_subject CHECK (user_id IS NOT NULL OR service_account_id IS NOT NULL)
);

CREATE INDEX oauth_access_tokens_user ON oauth_access_tokens (user_id) WHERE revoked_at IS NULL;

CREATE TABLE role_assignments (
    user_id UUID NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('super_admin')),
    created_at TIMESTAMPTZ NOT NULL,
    PRIMARY KEY (user_id, role)
);

CREATE TABLE security_events (
    id UUID PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL,
    event_type TEXT NOT NULL,
    result TEXT NOT NULL CHECK (result IN ('success', 'failure', 'denied')),
    actor_user_id UUID,
    target_user_id UUID,
    client_id TEXT,
    session_id UUID,
    request_id TEXT,
    ip INET,
    user_agent TEXT,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb
);

CREATE INDEX security_events_occurred ON security_events (occurred_at DESC);
CREATE INDEX security_events_target ON security_events (target_user_id, occurred_at DESC);
CREATE INDEX security_events_type ON security_events (event_type, occurred_at DESC);
CREATE INDEX security_events_request ON security_events (request_id);

CREATE TABLE auth_attempts (
    id BIGSERIAL PRIMARY KEY,
    occurred_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    kind TEXT NOT NULL,
    subject TEXT NOT NULL,
    ip INET,
    success BOOLEAN NOT NULL
);

CREATE INDEX auth_attempts_subject ON auth_attempts (kind, subject, occurred_at DESC);
CREATE INDEX auth_attempts_ip ON auth_attempts (kind, ip, occurred_at DESC);
CREATE INDEX users_created_at ON users (created_at DESC);

CREATE TABLE email_messages (
    id UUID PRIMARY KEY,
    to_address TEXT NOT NULL,
    subject TEXT NOT NULL,
    template TEXT NOT NULL,
    text_body TEXT NOT NULL,
    html_body TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('queued', 'sent', 'failed')),
    provider TEXT NOT NULL,
    provider_message_id TEXT,
    error TEXT,
    created_at TIMESTAMPTZ NOT NULL,
    sent_at TIMESTAMPTZ,
    attempt_count INTEGER NOT NULL DEFAULT 0
);

CREATE INDEX email_messages_status ON email_messages (status, created_at);
CREATE INDEX email_messages_recipient ON email_messages (to_address, created_at DESC);

CREATE TABLE social_transactions (
    id UUID PRIMARY KEY,
    provider TEXT NOT NULL CHECK (provider IN ('google', 'github')),
    state_hash BYTEA NOT NULL UNIQUE,
    verifier_ciphertext BYTEA NOT NULL,
    verifier_nonce BYTEA NOT NULL,
    nonce_hash BYTEA,
    mode TEXT NOT NULL CHECK (mode IN ('login', 'link')),
    user_id UUID,
    return_to TEXT,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ
);

CREATE TABLE oauth_consent_requests (
    id UUID PRIMARY KEY,
    client_id TEXT NOT NULL REFERENCES oauth_clients (id),
    redirect_uri TEXT NOT NULL,
    scopes TEXT[] NOT NULL,
    state TEXT NOT NULL,
    nonce TEXT,
    code_challenge TEXT NOT NULL,
    code_challenge_method TEXT NOT NULL,
    expires_at TIMESTAMPTZ NOT NULL,
    created_at TIMESTAMPTZ NOT NULL
);

-- Production redirect URIs only. Development startup adds loopback URIs.
INSERT INTO oauth_clients (
    id, name, client_type, redirect_uris, allowed_scopes, first_party, require_pkce, status, created_at
) VALUES
(
    'knotree-study',
    'Knotree Study',
    'public',
    ARRAY['https://study.knotree.com/auth/callback'],
    ARRAY['openid', 'profile', 'email', 'offline_access', 'study:read', 'study:write'],
    TRUE, TRUE, 'active', now()
),
(
    'knotree-registry',
    'Knotree Registry',
    'public',
    ARRAY['https://registry.knotree.com/auth/callback'],
    ARRAY['openid', 'profile', 'email', 'offline_access', 'registry:pull', 'registry:push'],
    TRUE, TRUE, 'active', now()
),
(
    'knotree-cloud',
    'Knotree Cloud',
    'public',
    ARRAY['https://cloud.knotree.com/auth/callback'],
    ARRAY['openid', 'profile', 'email', 'offline_access', 'cloud:read', 'cloud:manage'],
    TRUE, TRUE, 'active', now()
),
(
    'knotree-dashboard',
    'Knotree Dashboard',
    'public',
    ARRAY['https://dashboard.knotree.com/auth/callback'],
    ARRAY['openid', 'profile', 'email', 'offline_access'],
    TRUE, TRUE, 'active', now()
),
(
    'knotree-drive',
    'Knotree Drive',
    'public',
    ARRAY['https://drive.knotree.com/auth/callback'],
    ARRAY['openid', 'profile', 'email', 'offline_access'],
    TRUE, TRUE, 'active', now()
),
(
    'knotree-app',
    'Knotree App',
    'public',
    ARRAY['https://app.knotree.com/auth/callback'],
    ARRAY['openid', 'profile', 'email', 'offline_access'],
    TRUE, TRUE, 'active', now()
);
