use super::{apply_meta, ClientMeta, NewEvent};
use crate::email::{self, templates};
use crate::error::{is_unique_violation, AppError, AppResult};
use crate::security::password::{hash_password, normalize_email, validate_password};
use crate::security::random::random_token;
use crate::security::rate_limit::{self, Limit};
use crate::security::redirect::safe_return_to;
use crate::security::sha256;
use crate::state::AppState;
use chrono::{Duration, Utc};
use uuid::Uuid;

pub struct RegisterInput {
    pub username: String,
    pub email: String,
    pub password: String,
    pub password_confirm: String,
    /// Where to continue after the email is verified, usually the pending
    /// `/oauth/authorize` request of the service that sent the user here.
    pub return_to: Option<String>,
}

pub async fn register(
    state: &AppState,
    input: RegisterInput,
    meta: &ClientMeta,
) -> AppResult<Uuid> {
    if input.password != input.password_confirm {
        return Err(AppError::Validation("Passwords do not match."));
    }
    let email = normalize_email(&input.email)?;
    let username = super::identity::normalize_username(&input.username)?;
    validate_password(&input.password, &email)?;
    rate_limit::enforce(
        state,
        &Limit {
            kind: "register",
            subject: format!(
                "ip:{}",
                meta.ip
                    .map(|ip| ip.to_string())
                    .unwrap_or_else(|| "none".into())
            ),
            limit: 20,
            window_seconds: 3600,
        },
        meta.ip,
    )
    .await?;

    let password_hash = hash_password(&input.password, &state.config.argon)?;
    let now = Utc::now();
    let user_id = Uuid::now_v7();
    let email_id = Uuid::now_v7();
    let identity_id = Uuid::now_v7();
    let token = random_token()?;
    let challenge_metadata = verification_metadata(input.return_to.as_deref());
    let mut tx = state.db.begin().await?;
    super::identity::claim_username(&mut tx, &username, None).await?;
    super::identity::release_stale_claim(&mut tx, &email, state.config.verification_hours).await?;
    let inserted = sqlx::query(
        r#"
        INSERT INTO users (id, username, status, created_at, updated_at, password_changed_at, locale)
        VALUES ($1, $2, 'active', $3, $3, $3, $4)
        "#,
    )
    .bind(user_id)
    .bind(&username)
    .bind(now)
    .bind(meta.locale.as_str())
    .execute(&mut *tx)
    .await;
    if let Err(err) = inserted {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict("That username is already taken."));
        }
        return Err(AppError::from(err));
    }
    if let Err(err) = sqlx::query(
        r#"
        INSERT INTO user_emails (id, user_id, email, is_primary, created_at)
        VALUES ($1, $2, $3, TRUE, $4)
        "#,
    )
    .bind(email_id)
    .bind(user_id)
    .bind(&email)
    .bind(now)
    .execute(&mut *tx)
    .await
    {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict(
                "An account with this email already exists.",
            ));
        }
        return Err(AppError::from(err));
    }
    sqlx::query(
        r#"
        INSERT INTO identities (id, user_id, provider, provider_subject, email, email_verified, created_at)
        VALUES ($1, $2, 'password', $3, $4, FALSE, $5)
        "#,
    )
    .bind(identity_id)
    .bind(user_id)
    .bind(user_id.to_string())
    .bind(&email)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO password_credentials (identity_id, password_hash, updated_at) VALUES ($1, $2, $3)",
    )
    .bind(identity_id)
    .bind(&password_hash)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO email_challenges (id, user_id, email, purpose, code_hash, created_at, expires_at, metadata)
        VALUES ($1, $2, $3, 'email_verify', $4, $5, $6, $7)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(&email)
    .bind(sha256(token.as_bytes()))
    .bind(now)
    .bind(now + Duration::hours(state.config.verification_hours))
    .bind(&challenge_metadata)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("USER_REGISTERED", user_id);
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;

    let link = format!("{}/verify-email?token={token}", state.config.app_base_url);
    if let Err(err) =
        email::enqueue_and_send(state, &email, templates::verification(meta.locale, &link)).await
    {
        tracing::error!(error = %err, "verification email failed");
    }
    rate_limit::record(state, "register", &format!("email:{email}"), meta.ip, true).await?;
    metrics::counter!("auth_register_total").increment(1);
    Ok(user_id)
}

fn verification_metadata(return_to: Option<&str>) -> serde_json::Value {
    match return_to.and_then(safe_return_to) {
        Some(return_to) => serde_json::json!({ "return_to": return_to }),
        None => serde_json::json!({}),
    }
}

/// Verifies an email and returns the `return_to` saved when the link was sent.
pub async fn verify_email(
    state: &AppState,
    token: &str,
    meta: &ClientMeta,
) -> AppResult<Option<String>> {
    if token.len() < 20 || token.len() > 256 {
        return Err(AppError::Gone(
            "This verification link is invalid or expired.",
        ));
    }
    let hash = sha256(token.as_bytes());
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, Uuid, String, serde_json::Value)> = sqlx::query_as(
        r#"
        SELECT id, user_id, email, metadata
        FROM email_challenges
        WHERE purpose = 'email_verify' AND code_hash = $1 AND consumed_at IS NULL AND expires_at > now()
        FOR UPDATE
        "#,
    )
    .bind(&hash)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((challenge_id, user_id, email, metadata)) = row else {
        return Err(AppError::Gone(
            "This verification link is invalid or expired.",
        ));
    };
    let updated = sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE id = $1 AND consumed_at IS NULL",
    )
    .bind(challenge_id)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Gone(
            "This verification link is invalid or expired.",
        ));
    }
    sqlx::query("UPDATE user_emails SET verified_at = now() WHERE user_id = $1 AND email = $2")
        .bind(user_id)
        .bind(&email)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE identities SET email_verified = TRUE WHERE user_id = $1 AND provider = 'password' AND email = $2",
    )
    .bind(user_id)
    .bind(&email)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("EMAIL_VERIFIED", user_id);
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    Ok(metadata
        .get("return_to")
        .and_then(|value| value.as_str())
        .and_then(safe_return_to))
}

pub async fn resend_verification(
    state: &AppState,
    email_input: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let email = normalize_email(email_input).unwrap_or_default();
    rate_limit::enforce(
        state,
        &Limit {
            kind: "verify_resend",
            subject: format!("email:{email}"),
            limit: 3,
            window_seconds: 900,
        },
        meta.ip,
    )
    .await?;
    rate_limit::record(
        state,
        "verify_resend",
        &format!("email:{email}"),
        meta.ip,
        false,
    )
    .await?;
    let row: Option<(Uuid, Option<chrono::DateTime<Utc>>)> = sqlx::query_as(
        r#"
        SELECT u.id, e.verified_at
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE e.email = $1 AND u.status = 'active'
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    let Some((user_id, verified_at)) = row else {
        return Ok(());
    };
    if verified_at.is_some() {
        return Ok(());
    }
    let token = random_token()?;
    let now = Utc::now();
    let previous: Option<serde_json::Value> = sqlx::query_scalar(
        "SELECT metadata FROM email_challenges WHERE user_id = $1 AND purpose = 'email_verify' ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let challenge_metadata = verification_metadata(
        previous
            .as_ref()
            .and_then(|metadata| metadata.get("return_to"))
            .and_then(|value| value.as_str()),
    );
    sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE user_id = $1 AND purpose = 'email_verify' AND consumed_at IS NULL",
    )
    .bind(user_id)
    .execute(&state.db)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO email_challenges (id, user_id, email, purpose, code_hash, created_at, expires_at, metadata)
        VALUES ($1,$2,$3,'email_verify',$4,$5,$6,$7)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(&email)
    .bind(sha256(token.as_bytes()))
    .bind(now)
    .bind(now + Duration::hours(state.config.verification_hours))
    .bind(&challenge_metadata)
    .execute(&state.db)
    .await?;
    let link = format!("{}/verify-email?token={token}", state.config.app_base_url);
    email::enqueue_and_send(
        state,
        &email,
        templates::verification(crate::i18n::user_locale(&state.db, user_id).await, &link),
    )
    .await?;
    Ok(())
}
