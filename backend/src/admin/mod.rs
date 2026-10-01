use crate::auth::{self, ClientMeta};
use crate::email::{self, templates};
use crate::error::{AppError, AppResult};
use crate::security::random::random_token;
use crate::security::sha256;
use crate::state::AppState;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use std::sync::OnceLock;
use std::time::{Duration as StdDuration, Instant};
use tokio::sync::Mutex;
use uuid::Uuid;

static STATS_CACHE: OnceLock<Mutex<Option<(Instant, Value)>>> = OnceLock::new();

pub async fn stats(state: &AppState) -> AppResult<Value> {
    let cache = STATS_CACHE.get_or_init(|| Mutex::new(None));
    if let Some((cached_at, value)) = cache.lock().await.as_ref() {
        if cached_at.elapsed() < StdDuration::from_secs(15) {
            return Ok(value.clone());
        }
    }
    let users: (i64, i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE u.status <> 'pending_deletion'),
            COUNT(*) FILTER (WHERE e.verified_at IS NOT NULL AND u.status <> 'pending_deletion'),
            COUNT(*) FILTER (WHERE e.verified_at IS NULL AND u.status <> 'pending_deletion'),
            COUNT(*) FILTER (WHERE u.created_at >= date_trunc('day', now()) AND u.status <> 'pending_deletion'),
            COUNT(*) FILTER (WHERE u.created_at >= date_trunc('week', now()) AND u.status <> 'pending_deletion'),
            COUNT(*) FILTER (WHERE u.created_at >= date_trunc('month', now()) AND u.status <> 'pending_deletion')
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        "#,
    )
    .fetch_one(&state.db)
    .await?;
    let active_sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE revoked_at IS NULL AND expires_at > now()",
    )
    .fetch_one(&state.db)
    .await?;
    let mfa_users: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(DISTINCT user_id) FROM mfa_methods
        WHERE enabled_at IS NOT NULL AND disabled_at IS NULL
        "#,
    )
    .fetch_one(&state.db)
    .await?;
    let logins: (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(*) FILTER (WHERE event_type = 'LOGIN_SUCCESS' AND occurred_at >= date_trunc('day', now())),
            COUNT(*) FILTER (WHERE event_type = 'LOGIN_FAILED' AND occurred_at >= date_trunc('day', now()))
        FROM security_events
        "#,
    )
    .fetch_one(&state.db)
    .await?;
    let alerts: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*) FROM security_events
        WHERE event_type IN ('LOGIN_FAILED', 'MFA_FAILED', 'ACCOUNT_DISABLED')
          AND occurred_at >= now() - interval '1 day'
        "#,
    )
    .fetch_one(&state.db)
    .await?;
    let growth: Vec<(DateTime<Utc>, i64)> = sqlx::query_as(
        r#"
        SELECT date_trunc('day', created_at), COUNT(*)
        FROM users
        WHERE created_at >= now() - interval '30 days' AND status <> 'pending_deletion'
        GROUP BY 1
        ORDER BY 1
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    let value = json!({
        "users": {
            "total": users.0,
            "verified": users.1,
            "unverified": users.2,
            "created_today": users.3,
            "created_this_week": users.4,
            "created_this_month": users.5,
        },
        "active_sessions": active_sessions,
        "mfa_enabled_users": mfa_users,
        "logins_today": { "success": logins.0, "failure": logins.1 },
        "security_alerts_today": alerts,
        "daily_signups": growth.into_iter().map(|(day, count)| json!({"day": day, "count": count})).collect::<Vec<_>>(),
    });
    *cache.lock().await = Some((Instant::now(), value.clone()));
    Ok(value)
}

pub struct UserQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub verified: Option<bool>,
    pub mfa: Option<bool>,
    pub sort: String,
    pub order: String,
    pub limit: i64,
    pub offset: i64,
}

pub async fn list_users(state: &AppState, query: UserQuery) -> AppResult<Value> {
    let limit = query.limit.clamp(1, 100);
    let offset = query.offset.max(0);
    let sort = match query.sort.as_str() {
        "email" => "e.email",
        "last_login" => "u.last_login_at",
        _ => "u.created_at",
    };
    let direction = if query.order == "asc" { "ASC" } else { "DESC" };
    let search = query
        .q
        .as_deref()
        .map(|value| value.trim().to_lowercase())
        .filter(|value| !value.is_empty());
    let like = search.as_ref().map(|value| {
        format!(
            "%{}%",
            value
                .replace('\\', "\\\\")
                .replace('%', "\\%")
                .replace('_', "\\_")
        )
    });
    let sql = format!(
        r#"
        SELECT u.id, u.display_name, e.email, e.verified_at, u.created_at, u.last_login_at, u.status,
               EXISTS(
                   SELECT 1 FROM mfa_methods m
                   WHERE m.user_id = u.id AND m.enabled_at IS NOT NULL AND m.disabled_at IS NULL
               ) AS mfa
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE ($1::text IS NULL OR u.id::text = $1 OR e.email ILIKE $2 ESCAPE '\' OR u.display_name ILIKE $2 ESCAPE '\')
          AND ($3::text IS NULL OR u.status = $3)
          AND ($4::bool IS NULL OR ($4 = TRUE AND e.verified_at IS NOT NULL) OR ($4 = FALSE AND e.verified_at IS NULL))
          AND ($5::bool IS NULL OR $5 = EXISTS(
                SELECT 1 FROM mfa_methods m
                WHERE m.user_id = u.id AND m.enabled_at IS NOT NULL AND m.disabled_at IS NULL
          ))
        ORDER BY {sort} {direction} NULLS LAST
        LIMIT $6 OFFSET $7
        "#
    );
    let rows: Vec<(
        Uuid,
        Option<String>,
        String,
        Option<DateTime<Utc>>,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        String,
        bool,
    )> = sqlx::query_as(&sql)
        .bind(search.as_deref())
        .bind(like)
        .bind(query.status.as_deref())
        .bind(query.verified)
        .bind(query.mfa)
        .bind(limit + 1)
        .bind(offset)
        .fetch_all(&state.db)
        .await?;
    let has_more = rows.len() as i64 > limit;
    let items = rows
        .into_iter()
        .take(limit as usize)
        .map(|row| {
            json!({
                "id": row.0,
                "display_name": row.1,
                "email": row.2,
                "email_verified": row.3.is_some(),
                "created_at": row.4,
                "last_login_at": row.5,
                "status": row.6,
                "mfa_enabled": row.7,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"items": items, "has_more": has_more}))
}

pub async fn user_detail(state: &AppState, user_id: Uuid) -> AppResult<Value> {
    let row: Option<(Uuid, Option<String>, String, DateTime<Utc>, Option<DateTime<Utc>>, String, Option<DateTime<Utc>>, bool)> = sqlx::query_as(
        r#"
        SELECT u.id, u.display_name, e.email, u.created_at, u.last_login_at, u.status, e.verified_at, u.must_reset_password
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some(row) = row else {
        return Err(AppError::NotFound);
    };
    let identities: Vec<(String, Option<String>, bool)> = sqlx::query_as(
        "SELECT provider, email, email_verified FROM identities WHERE user_id = $1 ORDER BY created_at",
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    let sessions: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sessions WHERE user_id = $1 AND revoked_at IS NULL AND expires_at > now()",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    let mfa = auth::security_summary(state, user_id).await?;
    let events = auth::security_events(state, user_id, 20).await?;
    Ok(json!({
        "id": row.0,
        "display_name": row.1,
        "email": row.2,
        "created_at": row.3,
        "last_login_at": row.4,
        "status": row.5,
        "email_verified": row.6.is_some(),
        "must_reset_password": row.7,
        "identities": identities.into_iter().map(|(provider, email, verified)| json!({"provider": provider, "email": email, "email_verified": verified})).collect::<Vec<_>>(),
        "active_sessions": sessions,
        "mfa": mfa,
        "security_events": events,
    }))
}

pub async fn set_status(
    state: &AppState,
    actor: Uuid,
    user_id: Uuid,
    status: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    if status != "active" && status != "disabled" {
        return Err(AppError::Validation("Status must be active or disabled."));
    }
    if actor == user_id && status == "disabled" {
        return Err(AppError::Forbidden(
            "You cannot disable your own admin account.",
        ));
    }
    let current: Option<String> = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    let Some(current) = current else {
        return Err(AppError::NotFound);
    };
    if current == "pending_deletion" {
        return Err(AppError::Forbidden("This account is pending deletion."));
    }
    sqlx::query("UPDATE users SET status = $2, updated_at = now() WHERE id = $1")
        .bind(user_id)
        .bind(status)
        .execute(&state.db)
        .await?;
    if status == "disabled" {
        auth::revoke_all(&state.db, user_id, None, "admin_disable").await?;
    }
    let mut event = auth::NewEvent::success("ADMIN_ACTION", actor);
    event.target_user_id = Some(user_id);
    event.metadata = json!({"action": "set_status", "status": status});
    auth::apply_meta(&mut event, meta);
    auth::record(&state.db, event).await?;
    Ok(())
}

pub async fn revoke_user_sessions(
    state: &AppState,
    actor: Uuid,
    user_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<()> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE id = $1)")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }
    auth::revoke_all(&state.db, user_id, None, "admin_revoke").await?;
    let mut event = auth::NewEvent::success("ADMIN_ACTION", actor);
    event.target_user_id = Some(user_id);
    event.metadata = json!({"action": "revoke_sessions"});
    auth::apply_meta(&mut event, meta);
    auth::record(&state.db, event).await?;
    Ok(())
}

pub async fn force_password_reset(
    state: &AppState,
    actor: Uuid,
    user_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<()> {
    let row: Option<(String, String)> = sqlx::query_as(
        r#"
        SELECT u.status, e.email
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(&state.db)
    .await?;
    let Some((status, email)) = row else {
        return Err(AppError::NotFound);
    };
    if status != "active" {
        return Err(AppError::Forbidden(
            "Only active accounts can be asked to reset a password.",
        ));
    }
    sqlx::query("UPDATE users SET must_reset_password = TRUE, updated_at = now() WHERE id = $1")
        .bind(user_id)
        .execute(&state.db)
        .await?;
    auth::revoke_all(&state.db, user_id, None, "admin_force_reset").await?;
    let token = random_token()?;
    let now = Utc::now();
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
    let link = format!("{}/reset-password?token={token}", state.config.app_base_url);
    email::enqueue_and_send(
        state,
        &email,
        templates::password_reset(crate::i18n::user_locale(&state.db, user_id).await, &link),
    )
    .await?;
    let mut event = auth::NewEvent::success("ADMIN_ACTION", actor);
    event.target_user_id = Some(user_id);
    event.metadata = json!({"action": "force_password_reset"});
    auth::apply_meta(&mut event, meta);
    auth::record(&state.db, event).await?;
    Ok(())
}

pub async fn security_log(state: &AppState, filters: LogFilters) -> AppResult<Value> {
    let limit = filters.limit.clamp(1, 100);
    let rows: Vec<(
        Uuid,
        DateTime<Utc>,
        String,
        String,
        Option<Uuid>,
        Option<String>,
        Option<ipnetwork::IpNetwork>,
        Value,
    )> = sqlx::query_as(
        r#"
        SELECT id, occurred_at, event_type, result, target_user_id, request_id, ip, metadata
        FROM security_events
        WHERE ($1::text IS NULL OR event_type = $1)
          AND ($2::uuid IS NULL OR target_user_id = $2 OR actor_user_id = $2)
          AND ($3::text IS NULL OR result = $3)
          AND ($4::text IS NULL OR request_id = $4)
          AND ($5::timestamptz IS NULL OR occurred_at >= $5)
          AND ($6::timestamptz IS NULL OR occurred_at <= $6)
        ORDER BY occurred_at DESC
        LIMIT $7 OFFSET $8
        "#,
    )
    .bind(filters.event_type.as_deref())
    .bind(filters.user_id)
    .bind(filters.result.as_deref())
    .bind(filters.request_id.as_deref())
    .bind(filters.from)
    .bind(filters.to)
    .bind(limit + 1)
    .bind(filters.offset.max(0))
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 > limit;
    let items = rows
        .into_iter()
        .take(limit as usize)
        .map(|row| {
            json!({
                "id": row.0,
                "occurred_at": row.1,
                "event_type": row.2,
                "result": row.3,
                "user_id": row.4,
                "request_id": row.5,
                "ip": row.6.map(|ip| ip.to_string()),
                "metadata": row.7,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"items": items, "has_more": has_more}))
}

pub async fn email_log(state: &AppState, limit: i64, offset: i64) -> AppResult<Value> {
    let limit = limit.clamp(1, 100);
    let rows: Vec<(
        Uuid,
        String,
        String,
        String,
        String,
        DateTime<Utc>,
        Option<DateTime<Utc>>,
        i32,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT id, to_address, template, status, provider, created_at, sent_at, attempt_count, error
        FROM email_messages
        ORDER BY created_at DESC
        LIMIT $1 OFFSET $2
        "#,
    )
    .bind(limit + 1)
    .bind(offset.max(0))
    .fetch_all(&state.db)
    .await?;
    let has_more = rows.len() as i64 > limit;
    let items = rows
        .into_iter()
        .take(limit as usize)
        .map(|row| {
            json!({
                "id": row.0,
                "to": row.1,
                "template": row.2,
                "status": row.3,
                "provider": row.4,
                "created_at": row.5,
                "sent_at": row.6,
                "attempt_count": row.7,
                "error": row.8,
            })
        })
        .collect::<Vec<_>>();
    Ok(json!({"items": items, "has_more": has_more}))
}

pub struct LogFilters {
    pub event_type: Option<String>,
    pub user_id: Option<Uuid>,
    pub result: Option<String>,
    pub request_id: Option<String>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: i64,
    pub offset: i64,
}
