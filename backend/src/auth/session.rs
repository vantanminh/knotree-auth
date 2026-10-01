use super::{apply_meta, ClientMeta, NewEvent};
use crate::error::{AppError, AppResult};
use crate::security::random::random_token;
use crate::security::{sha256, ua};
use crate::state::AppState;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct CurrentSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub authenticated_at: DateTime<Utc>,
    pub elevated_at: Option<DateTime<Utc>>,
    pub expires_at: DateTime<Utc>,
    pub device_label: String,
    pub auth_methods: Vec<String>,
    pub mfa_satisfied: bool,
    pub is_admin: bool,
}

#[derive(Clone, Debug)]
pub struct IssuedSession {
    pub session: CurrentSession,
    pub token: String,
}

pub struct IssueParams<'a> {
    pub user_id: Uuid,
    pub meta: &'a ClientMeta,
    pub auth_methods: Vec<String>,
    pub mfa_satisfied: bool,
    pub elevated: bool,
    pub notify_new_device: bool,
}

pub async fn issue(state: &AppState, params: IssueParams<'_>) -> AppResult<IssuedSession> {
    let token = random_token()?;
    let token_hash = sha256(token.as_bytes());
    let now = Utc::now();
    let is_admin = is_super_admin(state, params.user_id).await?;
    let expires_at = if is_admin {
        now + Duration::hours(state.config.admin_session_hours)
    } else {
        now + Duration::hours(state.config.session_ttl_hours)
    };
    let id = Uuid::now_v7();
    let device = ua::device_label(params.meta.user_agent.as_deref());
    let elevated_at = if params.elevated { Some(now) } else { None };
    let mut tx = state.db.begin().await?;
    sqlx::query(
        r#"
        INSERT INTO sessions (
            id, user_id, token_hash, created_at, last_active_at, authenticated_at, elevated_at,
            expires_at, ip, user_agent, device_label, auth_methods, mfa_satisfied
        ) VALUES ($1,$2,$3,$4,$4,$4,$5,$6,$7,$8,$9,$10,$11)
        "#,
    )
    .bind(id)
    .bind(params.user_id)
    .bind(&token_hash)
    .bind(now)
    .bind(elevated_at)
    .bind(expires_at)
    .bind(params.meta.ip)
    .bind(params.meta.user_agent.as_deref().map(super::truncate_ua))
    .bind(&device)
    .bind(&params.auth_methods)
    .bind(params.mfa_satisfied)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE users SET last_login_at = $2, updated_at = $2 WHERE id = $1")
        .bind(params.user_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    let mut event = NewEvent::success("LOGIN_SUCCESS", params.user_id);
    event.session_id = Some(id);
    event.metadata = serde_json::json!({
        "methods": params.auth_methods,
        "device": device,
    });
    apply_meta(&mut event, params.meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;

    if params.notify_new_device {
        let _ = notify_if_new_device(state, params.user_id, &device, id, params.meta).await;
    }

    Ok(IssuedSession {
        token,
        session: CurrentSession {
            id,
            user_id: params.user_id,
            created_at: now,
            last_active_at: now,
            authenticated_at: now,
            elevated_at,
            expires_at,
            device_label: device,
            auth_methods: params.auth_methods,
            mfa_satisfied: params.mfa_satisfied,
            is_admin,
        },
    })
}

async fn notify_if_new_device(
    state: &AppState,
    user_id: Uuid,
    device: &str,
    session_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<()> {
    let prior: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM sessions
        WHERE user_id = $1 AND device_label = $2 AND id <> $3
          AND created_at > now() - interval '90 days'
        "#,
    )
    .bind(user_id)
    .bind(device)
    .bind(session_id)
    .fetch_one(&state.db)
    .await?;
    if prior > 0 {
        return Ok(());
    }
    let Some((email, _)) = super::primary_email(&state.db, user_id).await? else {
        return Ok(());
    };
    let when = Utc::now().format("%Y-%m-%d %H:%M UTC").to_string();
    let ip = meta
        .ip
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "unknown".into());
    let url = format!("{}/account/security", state.config.app_base_url);
    let message = crate::email::templates::new_login(
        crate::i18n::user_locale(&state.db, user_id).await,
        device,
        &when,
        &ip,
        &url,
    );
    if let Err(err) = crate::email::enqueue_and_send(state, &email, message).await {
        tracing::error!(error = %err, "new login alert was not sent");
    }
    Ok(())
}

pub struct LoadedSession {
    pub session: CurrentSession,
    pub refresh_cookie: bool,
}

pub async fn load(state: &AppState, token: &str) -> AppResult<Option<LoadedSession>> {
    if token.len() < 20 || token.len() > 128 {
        return Ok(None);
    }
    let hash = sha256(token.as_bytes());
    let row: Option<(
        Uuid,
        Uuid,
        DateTime<Utc>,
        DateTime<Utc>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
        String,
        Vec<String>,
        bool,
    )> = sqlx::query_as(
        r#"
        SELECT id, user_id, created_at, last_active_at, authenticated_at, elevated_at, expires_at,
               device_label, auth_methods, mfa_satisfied
        FROM sessions
        WHERE token_hash = $1 AND revoked_at IS NULL
        "#,
    )
    .bind(hash)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let now = Utc::now();
    if row.6 <= now {
        return Ok(None);
    }
    let is_admin = is_super_admin(state, row.1).await?;
    let idle = if is_admin {
        Duration::minutes(state.config.admin_idle_minutes)
    } else {
        Duration::hours(state.config.session_idle_hours)
    };
    if row.3 + idle <= now {
        let _ = revoke_id(&state.db, row.0, "idle").await;
        return Ok(None);
    }
    let mut expires_at = row.6;
    let mut refresh_cookie = false;
    let stale = row.3 + Duration::minutes(5) < now;
    let expiring_soon = !is_admin && expires_at <= now + Duration::hours(24);
    if stale || expiring_soon {
        if !is_admin {
            let extended = now + Duration::hours(state.config.session_ttl_hours);
            if extended > expires_at {
                expires_at = extended;
                refresh_cookie = true;
            }
        }
        sqlx::query(
            "UPDATE sessions SET last_active_at = $2, expires_at = $3 WHERE id = $1 AND revoked_at IS NULL",
        )
        .bind(row.0)
        .bind(now)
        .bind(expires_at)
        .execute(&state.db)
        .await?;
    }
    Ok(Some(LoadedSession {
        refresh_cookie,
        session: CurrentSession {
            id: row.0,
            user_id: row.1,
            created_at: row.2,
            last_active_at: now,
            authenticated_at: row.4,
            elevated_at: row.5,
            expires_at,
            device_label: row.7,
            auth_methods: row.8,
            mfa_satisfied: row.9,
            is_admin,
        },
    }))
}

pub fn require_step_up(state: &AppState, session: &CurrentSession) -> AppResult<()> {
    let Some(elevated) = session.elevated_at else {
        return Err(AppError::StepUpRequired);
    };
    if elevated + Duration::minutes(state.config.step_up_minutes) < Utc::now() {
        return Err(AppError::StepUpRequired);
    }
    Ok(())
}

pub async fn rotate(
    state: &AppState,
    session: &CurrentSession,
    meta: &ClientMeta,
) -> AppResult<IssuedSession> {
    revoke_id(&state.db, session.id, "rotation").await?;
    let mut issued = issue(
        state,
        IssueParams {
            user_id: session.user_id,
            meta,
            auth_methods: session.auth_methods.clone(),
            mfa_satisfied: session.mfa_satisfied,
            elevated: true,
            notify_new_device: false,
        },
    )
    .await?;
    issued.session.authenticated_at = session.authenticated_at;
    sqlx::query("UPDATE sessions SET authenticated_at = $2, elevated_at = $3 WHERE id = $1")
        .bind(issued.session.id)
        .bind(session.authenticated_at)
        .bind(Utc::now())
        .execute(&state.db)
        .await?;
    issued.session.elevated_at = Some(Utc::now());
    Ok(issued)
}

pub async fn revoke_id(db: &PgPool, session_id: Uuid, reason: &str) -> AppResult<()> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "UPDATE sessions SET revoked_at = now(), revoke_reason = $2 WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(session_id)
    .bind(reason)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE session_id = $1 AND revoked_at IS NULL",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE oauth_access_tokens SET revoked_at = now() WHERE session_id = $1 AND revoked_at IS NULL",
    )
    .bind(session_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn revoke_all(
    db: &PgPool,
    user_id: Uuid,
    except: Option<Uuid>,
    reason: &str,
) -> AppResult<()> {
    sqlx::query(
        r#"
        UPDATE sessions SET revoked_at = now(), revoke_reason = $3
        WHERE user_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR id <> $2)
        "#,
    )
    .bind(user_id)
    .bind(except)
    .bind(reason)
    .execute(db)
    .await?;
    sqlx::query(
        r#"
        UPDATE refresh_tokens SET revoked_at = now()
        WHERE user_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR session_id IS DISTINCT FROM $2)
        "#,
    )
    .bind(user_id)
    .bind(except)
    .execute(db)
    .await?;
    sqlx::query(
        r#"
        UPDATE oauth_access_tokens SET revoked_at = now()
        WHERE user_id = $1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR session_id IS DISTINCT FROM $2)
        "#,
    )
    .bind(user_id)
    .bind(except)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn is_super_admin(state: &AppState, user_id: Uuid) -> AppResult<bool> {
    if state.config.env.is_production() {
        if state.config.super_admin_user_id != Some(user_id) {
            return Ok(false);
        }
    } else if let Some(locked) = state.config.super_admin_user_id {
        if locked != user_id {
            return Ok(false);
        }
    }
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM role_assignments WHERE user_id = $1 AND role = 'super_admin')",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(exists)
}

pub async fn list_sessions(
    state: &AppState,
    user_id: Uuid,
    current: Uuid,
) -> AppResult<Vec<serde_json::Value>> {
    let rows: Vec<(Uuid, DateTime<Utc>, DateTime<Utc>, DateTime<Utc>, Option<ipnetwork::IpNetwork>, Option<String>, String, bool)> = sqlx::query_as(
        r#"
        SELECT id, created_at, last_active_at, expires_at, ip, user_agent, device_label, mfa_satisfied
        FROM sessions
        WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()
        ORDER BY last_active_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "id": row.0,
                "created_at": row.1,
                "last_active_at": row.2,
                "expires_at": row.3,
                "ip": row.4.map(|ip| ip.to_string()),
                "user_agent": row.5,
                "device": row.6,
                "mfa": row.7,
                "current": row.0 == current,
            })
        })
        .collect())
}

pub async fn revoke_owned(
    state: &AppState,
    user_id: Uuid,
    session_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<()> {
    let owner: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM sessions WHERE id = $1")
        .bind(session_id)
        .fetch_optional(&state.db)
        .await?;
    if owner != Some(user_id) {
        return Err(AppError::NotFound);
    }
    revoke_id(&state.db, session_id, "user").await?;
    let mut event = NewEvent::success("SESSION_REVOKED", user_id);
    event.session_id = Some(session_id);
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    Ok(())
}
