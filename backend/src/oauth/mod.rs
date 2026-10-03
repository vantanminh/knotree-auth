use crate::auth::{self, ClientMeta};
use crate::config::AppConfig;
use crate::error::{AppError, AppResult};
use crate::security::pkce::verify_s256;
use crate::security::random::random_token;
use crate::security::redirect::exact_redirect_allowed;
use crate::security::{ct_eq, sha256};
use crate::state::AppState;
use chrono::{DateTime, Duration, Utc};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use serde::Serialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug)]
pub enum OAuthFailure {
    InvalidRequest(&'static str),
    InvalidClient,
    InvalidGrant,
    UnauthorizedClient,
    UnsupportedGrant,
    InvalidScope,
}

impl OAuthFailure {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidRequest(_) => "invalid_request",
            Self::InvalidClient => "invalid_client",
            Self::InvalidGrant => "invalid_grant",
            Self::UnauthorizedClient => "unauthorized_client",
            Self::UnsupportedGrant => "unsupported_grant_type",
            Self::InvalidScope => "invalid_scope",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::InvalidRequest(msg) => msg,
            Self::InvalidClient => "Client authentication failed.",
            Self::InvalidGrant => "The authorization grant is invalid.",
            Self::UnauthorizedClient => "This client cannot use that grant.",
            Self::UnsupportedGrant => "Unsupported grant_type.",
            Self::InvalidScope => "The requested scope is not allowed.",
        }
    }
}

#[derive(Clone)]
pub struct Client {
    pub id: String,
    pub name: String,
    pub client_type: String,
    pub secret_hash: Option<Vec<u8>>,
    pub redirect_uris: Vec<String>,
    pub allowed_scopes: Vec<String>,
    pub first_party: bool,
    pub status: String,
}

pub struct AuthorizeInput {
    pub client_id: String,
    pub redirect_uri: String,
    pub response_type: String,
    pub scope: String,
    pub state: String,
    pub code_challenge: String,
    pub code_challenge_method: String,
    pub nonce: Option<String>,
}

pub struct ValidAuthorize {
    pub client: Client,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub state: String,
    pub nonce: Option<String>,
    pub code_challenge: String,
}

pub async fn load_client(state: &AppState, client_id: &str) -> AppResult<Option<Client>> {
    let row: Option<(String, String, String, Option<Vec<u8>>, Vec<String>, Vec<String>, bool, String)> = sqlx::query_as(
        r#"
        SELECT id, name, client_type, secret_hash, redirect_uris, allowed_scopes, first_party, status
        FROM oauth_clients WHERE id = $1
        "#,
    )
    .bind(client_id)
    .fetch_optional(&state.db)
    .await?;
    Ok(row.map(|row| Client {
        id: row.0,
        name: row.1,
        client_type: row.2,
        secret_hash: row.3,
        redirect_uris: row.4,
        allowed_scopes: row.5,
        first_party: row.6,
        status: row.7,
    }))
}

pub async fn validate_authorize(
    state: &AppState,
    input: AuthorizeInput,
) -> Result<ValidAuthorize, OAuthFailure> {
    if input.client_id.len() > 128 || input.redirect_uri.len() > 2048 {
        return Err(OAuthFailure::InvalidRequest(
            "Invalid authorization request.",
        ));
    }
    let client = load_client(state, &input.client_id)
        .await
        .map_err(|_| OAuthFailure::InvalidRequest("Invalid authorization request."))?
        .filter(|client| client.status == "active")
        .ok_or(OAuthFailure::InvalidClient)?;
    if !exact_redirect_allowed(&client.redirect_uris, &input.redirect_uri) {
        return Err(OAuthFailure::InvalidRequest(
            "redirect_uri is not registered for this client.",
        ));
    }
    if input.response_type != "code" {
        return Err(OAuthFailure::InvalidRequest("response_type must be code."));
    }
    if input.state.len() < 8 || input.state.len() > 512 {
        return Err(OAuthFailure::InvalidRequest("state is required."));
    }
    if input.code_challenge_method != "S256" || !(43..=128).contains(&input.code_challenge.len()) {
        return Err(OAuthFailure::InvalidRequest("PKCE S256 is required."));
    }
    if input.nonce.as_ref().is_some_and(|nonce| nonce.len() > 256) {
        return Err(OAuthFailure::InvalidRequest("nonce is too long."));
    }
    let scopes = parse_scopes(&input.scope, &client.allowed_scopes)?;
    Ok(ValidAuthorize {
        client,
        redirect_uri: input.redirect_uri,
        scopes,
        state: input.state,
        nonce: input.nonce,
        code_challenge: input.code_challenge,
    })
}

fn parse_scopes(scope: &str, allowed: &[String]) -> Result<Vec<String>, OAuthFailure> {
    if scope.len() > 512 {
        return Err(OAuthFailure::InvalidScope);
    }
    let mut scopes = Vec::new();
    for part in scope.split_whitespace() {
        if !allowed.iter().any(|item| item == part) {
            return Err(OAuthFailure::InvalidScope);
        }
        if !scopes.iter().any(|item| item == part) {
            scopes.push(part.to_string());
        }
    }
    if scopes.is_empty() {
        return Err(OAuthFailure::InvalidScope);
    }
    Ok(scopes)
}

pub async fn issue_code(
    state: &AppState,
    auth: &ValidAuthorize,
    user_id: Uuid,
    session_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<String> {
    let code = random_token()?;
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO oauth_authorization_codes (
            id, code_hash, client_id, user_id, session_id, redirect_uri, scopes, nonce,
            code_challenge, code_challenge_method, expires_at, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,'S256',$10,$11)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(sha256(code.as_bytes()))
    .bind(&auth.client.id)
    .bind(user_id)
    .bind(session_id)
    .bind(&auth.redirect_uri)
    .bind(&auth.scopes)
    .bind(&auth.nonce)
    .bind(&auth.code_challenge)
    .bind(now + Duration::seconds(state.config.auth_code_seconds))
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO oauth_consents (user_id, client_id, scopes, granted_at)
        VALUES ($1,$2,$3,$4)
        ON CONFLICT (user_id, client_id) DO UPDATE SET scopes = EXCLUDED.scopes, granted_at = EXCLUDED.granted_at
        "#,
    )
    .bind(user_id)
    .bind(&auth.client.id)
    .bind(&auth.scopes)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let mut event = auth::NewEvent::success("OAUTH_AUTHORIZED", user_id);
    event.client_id = Some(auth.client.id.clone());
    event.session_id = Some(session_id);
    event.metadata = json!({"scopes": auth.scopes});
    auth::apply_meta(&mut event, meta);
    auth::record(&mut *tx, event).await?;
    tx.commit().await?;
    Ok(code)
}

#[derive(Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: &'static str,
    pub expires_in: i64,
    pub refresh_token: Option<String>,
    pub id_token: Option<String>,
    pub scope: String,
}

pub struct CodeExchange {
    pub code: String,
    pub redirect_uri: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub code_verifier: String,
}

pub async fn exchange_code(
    state: &AppState,
    input: CodeExchange,
) -> Result<TokenResponse, OAuthFailure> {
    let client = authenticate_client(state, &input.client_id, input.client_secret.as_deref())
        .await
        .map_err(|_| OAuthFailure::InvalidClient)?;
    if !(43..=128).contains(&input.code_verifier.len()) {
        return Err(OAuthFailure::InvalidGrant);
    }
    let hash = sha256(input.code.as_bytes());
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| OAuthFailure::InvalidGrant)?;
    let row: Option<(Uuid, Uuid, Uuid, String, Vec<String>, Option<String>, String, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT id, user_id, session_id, redirect_uri, scopes, nonce, code_challenge, expires_at, used_at
        FROM oauth_authorization_codes
        WHERE code_hash = $1 AND client_id = $2
        FOR UPDATE
        "#,
    )
    .bind(&hash)
    .bind(&client.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| OAuthFailure::InvalidGrant)?;
    let Some((
        id,
        user_id,
        session_id,
        redirect_uri,
        scopes,
        nonce,
        challenge,
        expires_at,
        used_at,
    )) = row
    else {
        return Err(OAuthFailure::InvalidGrant);
    };
    if used_at.is_some() {
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = now() WHERE session_id = $1 AND client_id = $2",
        )
        .bind(session_id)
        .bind(&client.id)
        .execute(&mut *tx)
        .await
        .ok();
        sqlx::query("UPDATE oauth_access_tokens SET revoked_at = now() WHERE session_id = $1 AND client_id = $2")
            .bind(session_id)
            .bind(&client.id)
            .execute(&mut *tx)
            .await
            .ok();
        sqlx::query(
            "UPDATE sessions SET revoked_at = now(), revoke_reason = 'code_reuse' WHERE id = $1",
        )
        .bind(session_id)
        .execute(&mut *tx)
        .await
        .ok();
        tx.commit().await.ok();
        return Err(OAuthFailure::InvalidGrant);
    }
    if expires_at <= Utc::now()
        || redirect_uri != input.redirect_uri
        || !verify_s256(&input.code_verifier, &challenge)
    {
        return Err(OAuthFailure::InvalidGrant);
    }
    let updated = sqlx::query(
        "UPDATE oauth_authorization_codes SET used_at = now() WHERE id = $1 AND used_at IS NULL",
    )
    .bind(id)
    .execute(&mut *tx)
    .await
    .map_err(|_| OAuthFailure::InvalidGrant)?;
    if updated.rows_affected() != 1 {
        return Err(OAuthFailure::InvalidGrant);
    }
    let response = mint_user_tokens(
        &mut tx,
        state,
        &client,
        user_id,
        Some(session_id),
        &scopes,
        nonce.as_deref(),
    )
    .await
    .map_err(|_| OAuthFailure::InvalidGrant)?;
    tx.commit().await.map_err(|_| OAuthFailure::InvalidGrant)?;
    metrics::counter!("oauth_token_total", "grant" => "authorization_code").increment(1);
    Ok(response)
}

pub async fn refresh(
    state: &AppState,
    client_id: &str,
    client_secret: Option<&str>,
    refresh_token: &str,
) -> Result<TokenResponse, OAuthFailure> {
    let client = authenticate_client(state, client_id, client_secret)
        .await
        .map_err(|_| OAuthFailure::InvalidClient)?;
    let hash = sha256(refresh_token.as_bytes());
    let mut tx = state
        .db
        .begin()
        .await
        .map_err(|_| OAuthFailure::InvalidGrant)?;
    let row: Option<(
        Uuid,
        Uuid,
        Uuid,
        Option<Uuid>,
        Vec<String>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        r#"
        SELECT id, family_id, user_id, session_id, scopes, expires_at, used_at, revoked_at
        FROM refresh_tokens
        WHERE token_hash = $1 AND client_id = $2
        FOR UPDATE
        "#,
    )
    .bind(&hash)
    .bind(&client.id)
    .fetch_optional(&mut *tx)
    .await
    .map_err(|_| OAuthFailure::InvalidGrant)?;
    let Some((id, family_id, user_id, session_id, scopes, expires_at, used_at, revoked_at)) = row
    else {
        return Err(OAuthFailure::InvalidGrant);
    };
    if revoked_at.is_some() || expires_at <= Utc::now() {
        return Err(OAuthFailure::InvalidGrant);
    }
    if used_at.is_some() {
        sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE family_id = $1 AND revoked_at IS NULL")
            .bind(family_id)
            .execute(&mut *tx)
            .await
            .ok();
        if let Some(session_id) = session_id {
            sqlx::query("UPDATE sessions SET revoked_at = now(), revoke_reason = 'refresh_reuse' WHERE id = $1")
                .bind(session_id)
                .execute(&mut *tx)
                .await
                .ok();
        }
        tx.commit().await.ok();
        metrics::counter!("oauth_refresh_reuse_total").increment(1);
        return Err(OAuthFailure::InvalidGrant);
    }
    if let Some(session_id) = session_id {
        let alive: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM sessions WHERE id = $1 AND revoked_at IS NULL AND expires_at > now())",
        )
        .bind(session_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| OAuthFailure::InvalidGrant)?;
        if !alive {
            return Err(OAuthFailure::InvalidGrant);
        }
    }
    let new_id = Uuid::now_v7();
    let updated = sqlx::query("UPDATE refresh_tokens SET used_at = now(), replaced_by = $2 WHERE id = $1 AND used_at IS NULL")
        .bind(id)
        .bind(new_id)
        .execute(&mut *tx)
        .await
        .map_err(|_| OAuthFailure::InvalidGrant)?;
    if updated.rows_affected() != 1 {
        return Err(OAuthFailure::InvalidGrant);
    }
    let response = mint_user_tokens_with_family(
        &mut tx, state, &client, user_id, session_id, &scopes, None, family_id, new_id,
    )
    .await
    .map_err(|_| OAuthFailure::InvalidGrant)?;
    tx.commit().await.map_err(|_| OAuthFailure::InvalidGrant)?;
    metrics::counter!("oauth_token_total", "grant" => "refresh_token").increment(1);
    Ok(response)
}

pub async fn client_credentials(
    state: &AppState,
    client_id: &str,
    client_secret: Option<&str>,
    scope: &str,
) -> Result<TokenResponse, OAuthFailure> {
    let client = authenticate_client(state, client_id, client_secret)
        .await
        .map_err(|_| OAuthFailure::InvalidClient)?;
    if client.client_type != "service" {
        return Err(OAuthFailure::UnauthorizedClient);
    }
    let scopes = if scope.is_empty() {
        client.allowed_scopes.clone()
    } else {
        parse_scopes(scope, &client.allowed_scopes)?
    };
    let service_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM service_accounts WHERE client_id = $1 AND disabled_at IS NULL",
    )
    .bind(&client.id)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| OAuthFailure::InvalidClient)?;
    let Some(service_id) = service_id else {
        return Err(OAuthFailure::UnauthorizedClient);
    };
    let (access, expires_in) =
        insert_access_token(state, None, Some(service_id), &client.id, None, &scopes)
            .await
            .map_err(|_| OAuthFailure::InvalidGrant)?;
    metrics::counter!("oauth_token_total", "grant" => "client_credentials").increment(1);
    Ok(TokenResponse {
        access_token: access,
        token_type: "Bearer",
        expires_in,
        refresh_token: None,
        id_token: None,
        scope: scopes.join(" "),
    })
}

pub async fn revoke(
    state: &AppState,
    client_id: &str,
    client_secret: Option<&str>,
    token: &str,
) -> Result<(), OAuthFailure> {
    let client = authenticate_client(state, client_id, client_secret)
        .await
        .map_err(|_| OAuthFailure::InvalidClient)?;
    let hash = sha256(token.as_bytes());
    sqlx::query("UPDATE oauth_access_tokens SET revoked_at = now() WHERE token_hash = $1 AND client_id = $2 AND revoked_at IS NULL")
        .bind(&hash)
        .bind(&client.id)
        .execute(&state.db)
        .await
        .map_err(|_| OAuthFailure::InvalidClient)?;
    sqlx::query("UPDATE refresh_tokens SET revoked_at = now() WHERE token_hash = $1 AND client_id = $2 AND revoked_at IS NULL")
        .bind(&hash)
        .bind(&client.id)
        .execute(&state.db)
        .await
        .map_err(|_| OAuthFailure::InvalidClient)?;
    Ok(())
}

pub async fn introspect(
    state: &AppState,
    client_id: &str,
    client_secret: Option<&str>,
    token: &str,
) -> Result<Value, OAuthFailure> {
    let client = authenticate_client(state, client_id, client_secret)
        .await
        .map_err(|_| OAuthFailure::InvalidClient)?;
    let hash = sha256(token.as_bytes());
    let row: Option<(
        Option<Uuid>,
        Option<Uuid>,
        String,
        Vec<String>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        r#"
        SELECT user_id, service_account_id, client_id, scopes, expires_at, revoked_at
        FROM oauth_access_tokens WHERE token_hash = $1
        "#,
    )
    .bind(hash)
    .fetch_optional(&state.db)
    .await
    .map_err(|_| OAuthFailure::InvalidClient)?;
    let Some((user_id, service_id, token_client, scopes, expires_at, revoked_at)) = row else {
        return Ok(json!({"active": false}));
    };
    if revoked_at.is_some() || expires_at <= Utc::now() || token_client != client.id {
        return Ok(json!({"active": false}));
    }
    let sub = if let Some(user_id) = user_id {
        user_id.to_string()
    } else {
        format!("service:{}", service_id.unwrap_or_default())
    };
    Ok(json!({
        "active": true,
        "sub": sub,
        "client_id": token_client,
        "scope": scopes.join(" "),
        "exp": expires_at.timestamp(),
        "iss": state.config.issuer(),
    }))
}

pub async fn userinfo(state: &AppState, access_token: &str) -> AppResult<Value> {
    let hash = sha256(access_token.as_bytes());
    let row: Option<(Uuid, Vec<String>, DateTime<Utc>, Option<DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT user_id, scopes, expires_at, revoked_at
        FROM oauth_access_tokens
        WHERE token_hash = $1 AND user_id IS NOT NULL
        "#,
    )
    .bind(hash)
    .fetch_optional(&state.db)
    .await?;
    let Some((user_id, scopes, expires_at, revoked_at)) = row else {
        return Err(AppError::Unauthenticated);
    };
    if revoked_at.is_some() || expires_at <= Utc::now() {
        return Err(AppError::Unauthenticated);
    }
    let profile: Option<(Option<String>, String, Option<DateTime<Utc>>, String)> = sqlx::query_as(
        r#"
        SELECT u.display_name, e.email, e.verified_at, u.username
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE u.id = $1 AND u.status = 'active'
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((name, email, verified, username)) = profile else {
        return Err(AppError::Unauthenticated);
    };
    let mut body = json!({"sub": user_id});
    if scopes.iter().any(|scope| scope == "email") {
        body["email"] = json!(email);
        body["email_verified"] = json!(verified.is_some());
    }
    if scopes.iter().any(|scope| scope == "profile") {
        body["name"] = json!(name);
        body["preferred_username"] = json!(username);
    }
    Ok(body)
}

pub fn discovery(config: &AppConfig) -> Value {
    let issuer = config.issuer();
    json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/oauth/authorize"),
        "token_endpoint": format!("{issuer}/oauth/token"),
        "userinfo_endpoint": format!("{issuer}/oauth/userinfo"),
        "revocation_endpoint": format!("{issuer}/oauth/revoke"),
        "introspection_endpoint": format!("{issuer}/oauth/introspect"),
        "jwks_uri": format!("{issuer}/.well-known/jwks.json"),
        "response_types_supported": ["code"],
        "grant_types_supported": ["authorization_code", "refresh_token", "client_credentials"],
        "subject_types_supported": ["public"],
        "id_token_signing_alg_values_supported": ["RS256"],
        "code_challenge_methods_supported": ["S256"],
        "scopes_supported": ["openid", "profile", "email", "offline_access"],
        "token_endpoint_auth_methods_supported": ["client_secret_basic", "client_secret_post", "none"],
        "claims_supported": ["sub", "iss", "aud", "exp", "iat", "auth_time", "nonce", "email", "email_verified", "name", "preferred_username", "amr"]
    })
}

pub fn jwks(config: &AppConfig) -> Value {
    json!({
        "keys": config.jwt_keys.iter().map(|key| json!({
            "kty": "RSA",
            "use": "sig",
            "alg": "RS256",
            "kid": key.kid,
            "n": key.n,
            "e": key.e,
        })).collect::<Vec<_>>()
    })
}

async fn authenticate_client(
    state: &AppState,
    client_id: &str,
    secret: Option<&str>,
) -> AppResult<Client> {
    let client = load_client(state, client_id)
        .await?
        .filter(|client| client.status == "active")
        .ok_or(AppError::Unauthenticated)?;
    match client.client_type.as_str() {
        "public" => {
            if secret.is_some() {
                return Err(AppError::Unauthenticated);
            }
        }
        "confidential" | "service" => {
            let Some(secret) = secret else {
                return Err(AppError::Unauthenticated);
            };
            let Some(stored) = client.secret_hash.as_deref() else {
                return Err(AppError::Unauthenticated);
            };
            if !ct_eq(stored, &sha256(secret.as_bytes())) {
                return Err(AppError::Unauthenticated);
            }
        }
        _ => return Err(AppError::Unauthenticated),
    }
    Ok(client)
}

async fn mint_user_tokens(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    state: &AppState,
    client: &Client,
    user_id: Uuid,
    session_id: Option<Uuid>,
    scopes: &[String],
    nonce: Option<&str>,
) -> AppResult<TokenResponse> {
    mint_user_tokens_with_family(
        tx,
        state,
        client,
        user_id,
        session_id,
        scopes,
        nonce,
        Uuid::now_v7(),
        Uuid::now_v7(),
    )
    .await
}

async fn mint_user_tokens_with_family(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    state: &AppState,
    client: &Client,
    user_id: Uuid,
    session_id: Option<Uuid>,
    scopes: &[String],
    nonce: Option<&str>,
    family_id: Uuid,
    refresh_id: Uuid,
) -> AppResult<TokenResponse> {
    let access = random_token()?;
    let now = Utc::now();
    let expires_at = now + Duration::seconds(state.config.access_token_seconds);
    sqlx::query(
        r#"
        INSERT INTO oauth_access_tokens (
            id, token_hash, user_id, client_id, session_id, scopes, expires_at, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(sha256(access.as_bytes()))
    .bind(user_id)
    .bind(&client.id)
    .bind(session_id)
    .bind(scopes)
    .bind(expires_at)
    .bind(now)
    .execute(&mut **tx)
    .await?;
    let refresh_token = if scopes.iter().any(|scope| scope == "offline_access") {
        let refresh = random_token()?;
        sqlx::query(
            r#"
            INSERT INTO refresh_tokens (
                id, family_id, user_id, session_id, client_id, token_hash, scopes, expires_at, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
            "#,
        )
        .bind(refresh_id)
        .bind(family_id)
        .bind(user_id)
        .bind(session_id)
        .bind(&client.id)
        .bind(sha256(refresh.as_bytes()))
        .bind(scopes)
        .bind(now + Duration::days(state.config.refresh_token_days))
        .bind(now)
        .execute(&mut **tx)
        .await?;
        Some(refresh)
    } else {
        None
    };
    let id_token = if scopes.iter().any(|scope| scope == "openid") {
        Some(sign_id_token(state, client, user_id, session_id, scopes, nonce).await?)
    } else {
        None
    };
    Ok(TokenResponse {
        access_token: access,
        token_type: "Bearer",
        expires_in: state.config.access_token_seconds,
        refresh_token,
        id_token,
        scope: scopes.join(" "),
    })
}

async fn insert_access_token(
    state: &AppState,
    user_id: Option<Uuid>,
    service_account_id: Option<Uuid>,
    client_id: &str,
    session_id: Option<Uuid>,
    scopes: &[String],
) -> AppResult<(String, i64)> {
    let access = random_token()?;
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO oauth_access_tokens (
            id, token_hash, user_id, service_account_id, client_id, session_id, scopes, expires_at, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(sha256(access.as_bytes()))
    .bind(user_id)
    .bind(service_account_id)
    .bind(client_id)
    .bind(session_id)
    .bind(scopes)
    .bind(now + Duration::seconds(state.config.access_token_seconds))
    .bind(now)
    .execute(&state.db)
    .await?;
    Ok((access, state.config.access_token_seconds))
}

async fn sign_id_token(
    state: &AppState,
    client: &Client,
    user_id: Uuid,
    session_id: Option<Uuid>,
    scopes: &[String],
    nonce: Option<&str>,
) -> AppResult<String> {
    let profile: (Option<String>, String, Option<DateTime<Utc>>, String) = sqlx::query_as(
        r#"
        SELECT u.display_name, e.email, e.verified_at, u.username
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let auth_time: i64 = if let Some(session_id) = session_id {
        sqlx::query_scalar::<_, DateTime<Utc>>(
            "SELECT authenticated_at FROM sessions WHERE id = $1",
        )
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?
        .map(|time| time.timestamp())
        .unwrap_or_else(|| Utc::now().timestamp())
    } else {
        Utc::now().timestamp()
    };
    let amr: Vec<String> = if let Some(session_id) = session_id {
        sqlx::query_scalar("SELECT auth_methods FROM sessions WHERE id = $1")
            .bind(session_id)
            .fetch_optional(&state.db)
            .await?
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    let now = Utc::now().timestamp();
    let mut claims = json!({
        "iss": state.config.issuer(),
        "sub": user_id,
        "aud": client.id,
        "exp": now + 600,
        "iat": now,
        "auth_time": auth_time,
        "amr": amr,
    });
    if let Some(nonce) = nonce {
        claims["nonce"] = json!(nonce);
    }
    if scopes.iter().any(|scope| scope == "email") {
        claims["email"] = json!(profile.1);
        claims["email_verified"] = json!(profile.2.is_some());
    }
    if scopes.iter().any(|scope| scope == "profile") {
        claims["name"] = json!(profile.0);
        claims["preferred_username"] = json!(profile.3);
    }
    let key = state.config.active_jwt();
    let pem = key
        .private_pem
        .as_deref()
        .ok_or_else(|| AppError::internal("missing signing key"))?;
    let mut header = Header::new(Algorithm::RS256);
    header.kid = Some(key.kid.clone());
    encode(
        &header,
        &claims,
        &EncodingKey::from_rsa_pem(pem.as_bytes()).map_err(AppError::internal)?,
    )
    .map_err(AppError::internal)
}

pub fn basic_client(header: Option<&str>) -> Option<(String, String)> {
    let header = header?;
    let encoded = header.strip_prefix("Basic ")?;
    let bytes = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    let (id, secret) = text.split_once(':')?;
    Some((id.to_string(), secret.to_string()))
}
