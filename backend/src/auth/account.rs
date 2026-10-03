use super::session::{self, CurrentSession};
use super::{apply_meta, ClientMeta, NewEvent};
use crate::error::{AppError, AppResult};
use crate::i18n::Locale;
use crate::security::password::validate_display_name;
use crate::state::AppState;
use chrono::{DateTime, Utc};
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
        String,
        String,
        Option<DateTime<Utc>>,
    )> = sqlx::query_as(
        r#"
        SELECT u.id, u.display_name, u.status, u.created_at, e.verified_at, e.email, u.locale,
               u.username, u.username_changed_at
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
    let emails = super::identity::list_emails(state, session.user_id).await?;
    Ok(json!({
        "id": row.0,
        "username": row.7,
        "username_changed_at": row.8,
        "emails": emails,
        "display_name": row.1,
        "status": row.2,
        "created_at": row.3,
        "email": row.5,
        "locale": row.6,
        "email_verified": row.4.is_some(),
        "identities": identities.into_iter().map(|(provider, email)| json!({"provider": provider, "email": email})).collect::<Vec<_>>(),
        "mfa": mfa,
        "is_admin": session.is_admin,
    }))
}

pub async fn update_profile(
    state: &AppState,
    user_id: Uuid,
    display_name: Option<&str>,
    locale: Option<&str>,
    meta: &ClientMeta,
) -> AppResult<()> {
    let locale = locale
        .map(|value| {
            Locale::parse(value).ok_or(AppError::Validation("Choose a supported language."))
        })
        .transpose()?;
    if let Some(display_name) = display_name {
        let name = validate_display_name(display_name)?;
        sqlx::query("UPDATE users SET display_name = $2, updated_at = now() WHERE id = $1")
            .bind(user_id)
            .bind(name)
            .execute(&state.db)
            .await?;
    }
    if let Some(locale) = locale {
        sqlx::query("UPDATE users SET locale = $2, updated_at = now() WHERE id = $1")
            .bind(user_id)
            .bind(locale.as_str())
            .execute(&state.db)
            .await?;
    }
    let mut event = NewEvent::success("PROFILE_UPDATED", user_id);
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
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
    // Free the other addresses and the username for new accounts.
    sqlx::query("DELETE FROM user_emails WHERE user_id = $1 AND NOT is_primary")
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    let simple = session.user_id.simple().to_string();
    sqlx::query("UPDATE users SET username = $2 WHERE id = $1")
        .bind(session.user_id)
        .bind(format!("deleted-{}", &simple[simple.len() - 30..]))
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM username_holds WHERE user_id = $1")
        .bind(session.user_id)
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
