use super::session::{self, CurrentSession};
use super::{apply_meta, ClientMeta, NewEvent};
use crate::email::{self, templates};
use crate::error::{is_unique_violation, AppError, AppResult};
use crate::security::password::{normalize_email, validate_display_name};
use crate::security::random::random_token;
use crate::security::sha256;
use crate::state::AppState;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use uuid::Uuid;

pub async fn profile(state: &AppState, session: &CurrentSession) -> AppResult<Value> {
    let row: Option<(
        Uuid,
        Option<String>,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        String,
    )> = sqlx::query_as(
        r#"
        SELECT u.id, u.display_name, u.status, u.created_at, e.verified_at, e.email
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE u.id = $1
        "#,
    )
    .bind(session.user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Err(AppError::NotFound);
    };
    let identities: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT provider, email FROM identities WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(session.user_id)
    .fetch_all(&state.db)
    .await?;
    let mfa = super::mfa::security_summary(state, session.user_id).await?;
    Ok(json!({
        "id": row.0,
        "display_name": row.1,
        "status": row.2,
        "created_at": row.3,
        "email": row.5,
        "email_verified": row.4.is_some(),
        "identities": identities.into_iter().map(|(provider, email)| json!({"provider": provider, "email": email})).collect::<Vec<_>>(),
        "mfa": mfa,
        "is_admin": session.is_admin,
    }))
}

pub async fn update_profile(
    state: &AppState,
    user_id: Uuid,
    display_name: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let name = validate_display_name(display_name)?;
    sqlx::query("UPDATE users SET display_name = $2, updated_at = now() WHERE id = $1")
        .bind(user_id)
        .bind(name)
        .execute(&state.db)
        .await?;
    let mut event = NewEvent::success("PROFILE_UPDATED", user_id);
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    Ok(())
}

pub async fn request_email_change(
    state: &AppState,
    session: &CurrentSession,
    new_email: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    session::require_step_up(state, session)?;
    let email = normalize_email(new_email)?;
    let token = random_token()?;
    let now = Utc::now();
    sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE user_id = $1 AND purpose = 'email_change' AND consumed_at IS NULL",
    )
    .bind(session.user_id)
    .execute(&state.db)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO email_challenges (id, user_id, email, purpose, code_hash, created_at, expires_at, metadata)
        VALUES ($1,$2,$3,'email_change',$4,$5,$6,$7)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(session.user_id)
    .bind(&email)
    .bind(sha256(token.as_bytes()))
    .bind(now)
    .bind(now + Duration::hours(state.config.verification_hours))
    .bind(json!({"new_email": email}))
    .execute(&state.db)
    .await?;
    let link = format!("{}/verify-email?token={token}", state.config.app_base_url);
    email::enqueue_and_send(state, &email, templates::verification(&link)).await?;
    let mut event = NewEvent::success("EMAIL_CHANGE_REQUESTED", session.user_id);
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    Ok(())
}

pub async fn confirm_email_change(
    state: &AppState,
    token: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let hash = sha256(token.as_bytes());
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, user_id, email FROM email_challenges
        WHERE purpose = 'email_change' AND code_hash = $1 AND consumed_at IS NULL AND expires_at > now()
        FOR UPDATE
        "#,
    )
    .bind(hash)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, user_id, email)) = row else {
        return Err(AppError::Gone(
            "This verification link is invalid or expired.",
        ));
    };
    let old: Option<String> =
        sqlx::query_scalar("SELECT email FROM user_emails WHERE user_id = $1 AND is_primary")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    if let Err(err) = sqlx::query(
        "UPDATE user_emails SET email = $2, verified_at = now() WHERE user_id = $1 AND is_primary",
    )
    .bind(user_id)
    .bind(&email)
    .execute(&mut *tx)
    .await
    {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict(
                "An account with this email already exists.",
            ));
        }
        return Err(err.into());
    }
    sqlx::query(
        "UPDATE identities SET provider_subject = $2, email = $2, email_verified = TRUE WHERE user_id = $1 AND provider = 'password'",
    )
    .bind(user_id)
    .bind(&email)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE email_challenges SET consumed_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    let mut event = NewEvent::success("EMAIL_CHANGED", user_id);
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    if let Some(old) = old {
        let _ = email::enqueue_and_send(
            state,
            &old,
            templates::security_alert(
                "The email address on your Knotree account was changed.",
                &format!("{}/account/security", state.config.app_base_url),
            ),
        )
        .await;
    }
    Ok(())
}

pub async fn delete_account(
    state: &AppState,
    session: &CurrentSession,
    meta: &ClientMeta,
) -> AppResult<()> {
    session::require_step_up(state, session)?;
    if session::is_super_admin(state, session.user_id).await? {
        return Err(AppError::Forbidden(
            "The admin account cannot be deleted here.",
        ));
    }
    let replacement = format!("deleted+{}@users.knotree.invalid", session.user_id);
    let mut tx = state.db.begin().await?;
    sqlx::query(
        r#"
        UPDATE users
        SET status = 'pending_deletion', display_name = 'Deleted user', deletion_requested_at = now(), updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(session.user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE user_emails SET email = $2, verified_at = NULL WHERE user_id = $1 AND is_primary",
    )
    .bind(session.user_id)
    .bind(&replacement)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM password_credentials WHERE identity_id IN (SELECT id FROM identities WHERE user_id = $1)",
    )
    .bind(session.user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM mfa_methods WHERE user_id = $1")
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    let mut event = NewEvent::success("ACCOUNT_DELETION_REQUESTED", session.user_id);
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    session::revoke_all(&state.db, session.user_id, None, "deletion").await?;
    Ok(())
}

pub async fn security_events(state: &AppState, user_id: Uuid, limit: i64) -> AppResult<Vec<Value>> {
    let limit = limit.clamp(1, 100);
    let rows: Vec<(Uuid, DateTime<Utc>, String, String, Option<String>, Value)> = sqlx::query_as(
        r#"
        SELECT id, occurred_at, event_type, result, user_agent, metadata
        FROM security_events
        WHERE target_user_id = $1
        ORDER BY occurred_at DESC
        LIMIT $2
        "#,
    )
    .bind(user_id)
    .bind(limit)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            json!({
                "id": row.0,
                "occurred_at": row.1,
                "event_type": row.2,
                "result": row.3,
                "user_agent": row.4,
                "metadata": row.5,
            })
        })
        .collect())
}

pub async fn step_up_password(
    state: &AppState,
    session: &CurrentSession,
    password: &str,
) -> AppResult<bool> {
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
        return Ok(false);
    };
    Ok(crate::security::password::verify_password(
        password, &stored,
    ))
}
