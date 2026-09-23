use super::login::enabled_mfa_methods;
use super::session::{self, CurrentSession};
use super::{apply_meta, primary_email, ClientMeta, NewEvent};
use crate::email::{self, templates};
use crate::error::{AppError, AppResult};
use crate::security::password::{
    hash_password, normalize_email, validate_password, verify_password,
};
use crate::security::random::random_token;
use crate::security::rate_limit::{self, Limit};
use crate::security::sha256;
use crate::state::AppState;
use chrono::{Duration, Utc};
use uuid::Uuid;

pub async fn request_reset(
    state: &AppState,
    email_input: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let email = normalize_email(email_input).unwrap_or_else(|_| "invalid@invalid.invalid".into());
    rate_limit::enforce(
        state,
        &Limit {
            kind: "password_reset",
            subject: format!("email:{email}"),
            limit: 5,
            window_seconds: 3600,
        },
        meta.ip,
    )
    .await?;
    rate_limit::record(
        state,
        "password_reset",
        &format!("email:{email}"),
        meta.ip,
        false,
    )
    .await?;
    let row: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT u.id
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        JOIN identities i ON i.user_id = u.id AND i.provider = 'password'
        WHERE e.email = $1 AND u.status = 'active'
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    let Some(user_id) = row else {
        return Ok(());
    };
    let token = random_token()?;
    let now = Utc::now();
    sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE user_id = $1 AND purpose = 'password_reset' AND consumed_at IS NULL",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO email_challenges (id, user_id, email, purpose, code_hash, created_at, expires_at)
        VALUES ($1,$2,$3,'password_reset',$4,$5,$6)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(&email)
    .bind(sha256(token.as_bytes()))
    .bind(now)
    .bind(now + Duration::minutes(state.config.reset_minutes))
    .execute(&state.db)
    .await?;
    let mut event = NewEvent::success("PASSWORD_RESET_REQUESTED", user_id);
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    let link = format!("{}/reset-password?token={token}", state.config.app_base_url);
    email::enqueue_and_send(state, &email, templates::password_reset(&link)).await?;
    metrics::counter!("auth_password_reset_requested_total").increment(1);
    Ok(())
}

pub async fn reset_password(
    state: &AppState,
    token: &str,
    password: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    if token.len() < 20 {
        return Err(AppError::Gone("This reset link is invalid or expired."));
    }
    let hash = sha256(token.as_bytes());
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, Uuid, String, i32, i32)> = sqlx::query_as(
        r#"
        SELECT id, user_id, email, attempt_count, max_attempts
        FROM email_challenges
        WHERE purpose = 'password_reset' AND code_hash = $1 AND consumed_at IS NULL AND expires_at > now()
        FOR UPDATE
        "#,
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((challenge_id, user_id, email, attempts, max_attempts)) = row else {
        return Err(AppError::Gone("This reset link is invalid or expired."));
    };
    if attempts >= max_attempts {
        return Err(AppError::Gone("This reset link is invalid or expired."));
    }
    if let Err(err) = validate_password(password, &email) {
        sqlx::query("UPDATE email_challenges SET attempt_count = attempt_count + 1 WHERE id = $1")
            .bind(challenge_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(err);
    }
    let password_hash = hash_password(password, &state.config.argon)?;
    let now = Utc::now();
    sqlx::query("UPDATE email_challenges SET consumed_at = now() WHERE id = $1")
        .bind(challenge_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        UPDATE password_credentials pc
        SET password_hash = $2, updated_at = $3
        FROM identities i
        WHERE i.user_id = $1 AND i.provider = 'password' AND pc.identity_id = i.id
        "#,
    )
    .bind(user_id)
    .bind(&password_hash)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE users SET password_changed_at = $2, updated_at = $2, must_reset_password = FALSE WHERE id = $1",
    )
    .bind(user_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("PASSWORD_CHANGED", user_id);
    event.metadata = serde_json::json!({"via": "reset"});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    session::revoke_all(&state.db, user_id, None, "password_reset").await?;
    let _ = email::enqueue_and_send(
        state,
        &email,
        templates::security_alert(
            "The password for your Knotree account was reset. Other sessions were signed out.",
            &format!("{}/account/security", state.config.app_base_url),
        ),
    )
    .await;
    Ok(())
}

pub async fn change_password(
    state: &AppState,
    session: &CurrentSession,
    current_password: &str,
    new_password: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let (email, _) = primary_email(&state.db, session.user_id)
        .await?
        .ok_or(AppError::Validation("No email is available."))?;
    validate_password(new_password, &email)?;
    let stored: Option<String> = sqlx::query_scalar(
        r#"
        SELECT pc.password_hash
        FROM password_credentials pc
        JOIN identities i ON i.id = pc.identity_id
        WHERE i.user_id = $1 AND i.provider = 'password'
        "#,
    )
    .bind(session.user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(stored) = stored else {
        return Err(AppError::Validation("This account has no password yet."));
    };
    if !verify_password(current_password, &stored) {
        return Err(AppError::InvalidCredentials);
    }
    if enabled_mfa_methods(state, session.user_id)
        .await?
        .iter()
        .any(|m| m == "totp")
        && !session.mfa_satisfied
        && session::require_step_up(state, session).is_err()
    {
        return Err(AppError::StepUpRequired);
    }
    let password_hash = hash_password(new_password, &state.config.argon)?;
    let now = Utc::now();
    sqlx::query(
        r#"
        UPDATE password_credentials pc
        SET password_hash = $2, updated_at = $3
        FROM identities i
        WHERE i.user_id = $1 AND i.provider = 'password' AND pc.identity_id = i.id
        "#,
    )
    .bind(session.user_id)
    .bind(password_hash)
    .bind(now)
    .execute(&state.db)
    .await?;
    sqlx::query("UPDATE users SET password_changed_at = $2, updated_at = $2, must_reset_password = FALSE WHERE id = $1")
        .bind(session.user_id)
        .bind(now)
        .execute(&state.db)
        .await?;
    session::revoke_all(
        &state.db,
        session.user_id,
        Some(session.id),
        "password_change",
    )
    .await?;
    let mut event = NewEvent::success("PASSWORD_CHANGED", session.user_id);
    event.metadata = serde_json::json!({"via": "change"});
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    let _ = email::enqueue_and_send(
        state,
        &email,
        templates::security_alert(
            "The password for your Knotree account was changed.",
            &format!("{}/account/security", state.config.app_base_url),
        ),
    )
    .await;
    Ok(())
}
