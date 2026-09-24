use super::session::{self, IssueParams, IssuedSession};
use super::{apply_meta, ClientMeta, NewEvent};
use crate::email::{self, templates};
use crate::error::{AppError, AppResult};
use crate::security::password::{
    hash_password, mask_email, needs_rehash, normalize_email, verify_password,
};
use crate::security::random::{email_otp, random_token};
use crate::security::rate_limit::{self, Limit};
use crate::security::sha256;
use crate::state::AppState;
use chrono::{Duration, Utc};
use uuid::Uuid;

pub enum LoginResult {
    Session(IssuedSession),
    Mfa {
        token: String,
        methods: Vec<String>,
        masked_email: String,
    },
}

pub async fn login(
    state: &AppState,
    email_input: &str,
    password: &str,
    meta: &ClientMeta,
) -> AppResult<LoginResult> {
    let email = normalize_email(email_input).unwrap_or_else(|_| "invalid@invalid.invalid".into());
    let subject = format!("email:{email}");
    rate_limit::enforce(
        state,
        &Limit {
            kind: "login",
            subject: subject.clone(),
            limit: 8,
            window_seconds: 900,
        },
        meta.ip,
    )
    .await?;

    let row: Option<(Uuid, String, String, bool)> = sqlx::query_as(
        r#"
        SELECT u.id, u.status, pc.password_hash, u.must_reset_password
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        JOIN identities i ON i.user_id = u.id AND i.provider = 'password'
        JOIN password_credentials pc ON pc.identity_id = i.id
        WHERE e.email = $1
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;

    let (user_id, status, stored_hash, must_reset) = match &row {
        Some(row) => (row.0, row.1.clone(), row.2.clone(), row.3),
        None => (
            Uuid::nil(),
            "missing".into(),
            state.config.dummy_password_hash.clone(),
            false,
        ),
    };
    let password_ok = verify_password(password, &stored_hash);
    if row.is_none() || !password_ok {
        rate_limit::record(state, "login", &subject, meta.ip, false).await?;
        let mut event = NewEvent {
            event_type: "LOGIN_FAILED",
            result: "failure",
            actor_user_id: None,
            target_user_id: if row.is_some() { Some(user_id) } else { None },
            client_id: None,
            session_id: None,
            request_id: None,
            ip: None,
            user_agent: None,
            metadata: serde_json::json!({}),
        };
        apply_meta(&mut event, meta);
        super::record(&state.db, event).await?;
        metrics::counter!("auth_login_total", "result" => "failure").increment(1);
        return Err(AppError::InvalidCredentials);
    }
    if status != "active" {
        return Err(AppError::AccountDisabled);
    }
    if must_reset {
        return Err(AppError::PasswordResetRequired);
    }
    if needs_rehash(&stored_hash, &state.config.argon) {
        let new_hash = hash_password(password, &state.config.argon)?;
        sqlx::query(
            r#"
            UPDATE password_credentials pc
            SET password_hash = $2, updated_at = now()
            FROM identities i
            WHERE i.user_id = $1 AND i.provider = 'password' AND pc.identity_id = i.id
            "#,
        )
        .bind(user_id)
        .bind(new_hash)
        .execute(&state.db)
        .await?;
    }

    let methods = enabled_mfa_methods(state, user_id).await?;
    if !methods.is_empty() {
        let token = random_token()?;
        let now = Utc::now();
        sqlx::query(
            r#"
            INSERT INTO login_transactions (id, user_id, token_hash, created_at, expires_at, ip, user_agent)
            VALUES ($1,$2,$3,$4,$5,$6,$7)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(sha256(token.as_bytes()))
        .bind(now)
        .bind(now + Duration::minutes(5))
        .bind(meta.ip)
        .bind(meta.user_agent.as_deref().map(super::truncate_ua))
        .execute(&state.db)
        .await?;
        if methods.iter().any(|method| method == "email")
            && !methods.iter().any(|method| method == "totp")
        {
            send_email_otp(state, user_id, &email).await?;
        }
        let mut event = NewEvent::success("MFA_CHALLENGE_CREATED", user_id);
        event.metadata = serde_json::json!({ "methods": methods });
        apply_meta(&mut event, meta);
        super::record(&state.db, event).await?;
        return Ok(LoginResult::Mfa {
            token,
            methods,
            masked_email: mask_email(&email),
        });
    }

    rate_limit::record(state, "login", &subject, meta.ip, true).await?;
    metrics::counter!("auth_login_total", "result" => "success").increment(1);
    let issued = session::issue(
        state,
        IssueParams {
            user_id,
            meta,
            auth_methods: vec!["password".into()],
            mfa_satisfied: false,
            elevated: true,
            notify_new_device: true,
        },
    )
    .await?;
    Ok(LoginResult::Session(issued))
}

pub async fn enabled_mfa_methods(state: &AppState, user_id: Uuid) -> AppResult<Vec<String>> {
    let rows: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT m.method
        FROM mfa_methods m
        LEFT JOIN totp_credentials t ON t.mfa_method_id = m.id
        WHERE m.user_id = $1
          AND m.enabled_at IS NOT NULL
          AND m.disabled_at IS NULL
          AND (m.method <> 'totp' OR t.confirmed_at IS NOT NULL)
        ORDER BY m.method
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

pub async fn send_email_otp(state: &AppState, user_id: Uuid, email: &str) -> AppResult<()> {
    rate_limit::enforce(
        state,
        &Limit {
            kind: "mfa_resend",
            subject: format!("user:{user_id}"),
            limit: 3,
            window_seconds: 900,
        },
        None,
    )
    .await?;
    rate_limit::record(state, "mfa_resend", &format!("user:{user_id}"), None, false).await?;
    let code = email_otp()?;
    let now = Utc::now();
    sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE user_id = $1 AND purpose = 'email_mfa' AND consumed_at IS NULL",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO email_challenges (id, user_id, email, purpose, code_hash, created_at, expires_at, max_attempts)
        VALUES ($1,$2,$3,'email_mfa',$4,$5,$6,5)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(email)
    .bind(sha256(code.as_bytes()))
    .bind(now)
    .bind(now + Duration::seconds(state.config.email_otp_seconds))
    .execute(&state.db)
    .await?;
    email::enqueue_and_send(
        state,
        email,
        templates::mfa_code(&code, state.config.email_otp_seconds / 60),
    )
    .await?;
    metrics::counter!("auth_mfa_challenge_total", "method" => "email").increment(1);
    Ok(())
}

pub async fn send_login_email_otp(state: &AppState, mfa_token: &str) -> AppResult<String> {
    let user_id = load_login_user(state, mfa_token).await?;
    let methods = enabled_mfa_methods(state, user_id).await?;
    if !methods.iter().any(|method| method == "email") {
        return Err(AppError::Validation("Email codes are not enabled."));
    }
    let Some((email, verified)) = super::primary_email(&state.db, user_id).await? else {
        return Err(AppError::Validation("No email is available."));
    };
    if verified.is_none() {
        return Err(AppError::Validation(
            "Verify your email before using email codes.",
        ));
    }
    send_email_otp(state, user_id, &email).await?;
    Ok(mask_email(&email))
}

pub async fn load_login_user(state: &AppState, mfa_token: &str) -> AppResult<Uuid> {
    if mfa_token.len() < 20 {
        return Err(AppError::Gone("This sign-in attempt expired. Start again."));
    }
    let row: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT user_id FROM login_transactions
        WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > now()
        "#,
    )
    .bind(sha256(mfa_token.as_bytes()))
    .fetch_optional(&state.db)
    .await?;
    row.ok_or(AppError::Gone("This sign-in attempt expired. Start again."))
}
