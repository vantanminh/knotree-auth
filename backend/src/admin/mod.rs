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
static ANALYTICS_CACHE: OnceLock<Mutex<Vec<(i64, Instant, Value)>>> = OnceLock::new();

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

/// Range accepted by the analytics endpoint; anything else falls back to 30 days.
pub fn analytics_days(requested: Option<i64>) -> i64 {
    requested
        .filter(|days| matches!(days, 7 | 30 | 90))
        .unwrap_or(30)
}

pub async fn analytics(state: &AppState, days: i64) -> AppResult<Value> {
    let cache = ANALYTICS_CACHE.get_or_init(|| Mutex::new(Vec::new()));
    if let Some((_, _, value)) = cache
        .lock()
        .await
        .iter()
        .find(|(d, at, _)| *d == days && at.elapsed() < StdDuration::from_secs(60))
    {
        return Ok(value.clone());
    }

    // One row per UTC day in the range, zero-filled.
    let series: Vec<(DateTime<Utc>, i64, i64, i64, i64)> = sqlx::query_as(
        r#"
        WITH days AS (
            SELECT generate_series(
                date_trunc('day', now() AT TIME ZONE 'UTC') - make_interval(days => $1 - 1),
                date_trunc('day', now() AT TIME ZONE 'UTC'),
                interval '1 day'
            ) AT TIME ZONE 'UTC' AS day
        ),
        signups AS (
            SELECT date_trunc('day', created_at AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' AS day, COUNT(*) AS n
            FROM users
            WHERE created_at >= (SELECT MIN(day) FROM days) AND status <> 'pending_deletion'
            GROUP BY 1
        ),
        logins AS (
            SELECT date_trunc('day', occurred_at AT TIME ZONE 'UTC') AT TIME ZONE 'UTC' AS day,
                COUNT(*) FILTER (WHERE event_type = 'LOGIN_SUCCESS') AS ok,
                COUNT(*) FILTER (WHERE event_type = 'LOGIN_FAILED') AS failed,
                COUNT(DISTINCT target_user_id) FILTER (WHERE event_type = 'LOGIN_SUCCESS') AS active
            FROM security_events
            WHERE event_type IN ('LOGIN_SUCCESS', 'LOGIN_FAILED')
              AND occurred_at >= (SELECT MIN(day) FROM days)
            GROUP BY 1
        )
        SELECT d.day,
            COALESCE(s.n, 0),
            COALESCE(l.ok, 0),
            COALESCE(l.failed, 0),
            COALESCE(l.active, 0)
        FROM days d
        LEFT JOIN signups s ON s.day = d.day
        LEFT JOIN logins l ON l.day = d.day
        ORDER BY d.day
        "#,
    )
    .bind(days as i32)
    .fetch_all(&state.db)
    .await?;

    let active: (i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT
            COUNT(DISTINCT target_user_id) FILTER (WHERE occurred_at >= now() - interval '1 day'),
            COUNT(DISTINCT target_user_id) FILTER (WHERE occurred_at >= now() - interval '7 days'),
            COUNT(DISTINCT target_user_id)
        FROM security_events
        WHERE event_type = 'LOGIN_SUCCESS' AND occurred_at >= now() - interval '30 days'
        "#,
    )
    .fetch_one(&state.db)
    .await?;

    let login_methods: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT method, COUNT(*)
        FROM security_events,
            jsonb_array_elements_text(
                CASE WHEN jsonb_typeof(metadata->'methods') = 'array' THEN metadata->'methods' ELSE '[]'::jsonb END
            ) AS method
        WHERE event_type = 'LOGIN_SUCCESS' AND occurred_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') - make_interval(days => $1 - 1)) AT TIME ZONE 'UTC'
        GROUP BY method
        ORDER BY 2 DESC, 1
        "#,
    )
    .bind(days as i32)
    .fetch_all(&state.db)
    .await?;

    let providers: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT i.provider, COUNT(DISTINCT i.user_id)
        FROM identities i
        JOIN users u ON u.id = i.user_id AND u.status <> 'pending_deletion'
        GROUP BY 1
        ORDER BY 2 DESC, 1
        "#,
    )
    .fetch_all(&state.db)
    .await?;

    let event_types: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT event_type, COUNT(*)
        FROM security_events
        WHERE occurred_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') - make_interval(days => $1 - 1)) AT TIME ZONE 'UTC'
        GROUP BY 1
        ORDER BY 2 DESC, 1
        LIMIT 10
        "#,
    )
    .bind(days as i32)
    .fetch_all(&state.db)
    .await?;

    let clients: Vec<(String, i64)> = sqlx::query_as(
        r#"
        SELECT client_id, COUNT(*)
        FROM security_events
        WHERE event_type = 'OAUTH_AUTHORIZED' AND client_id IS NOT NULL
          AND occurred_at >= (date_trunc('day', now() AT TIME ZONE 'UTC') - make_interval(days => $1 - 1)) AT TIME ZONE 'UTC'
        GROUP BY 1
        ORDER BY 2 DESC, 1
        LIMIT 10
        "#,
    )
    .bind(days as i32)
    .fetch_all(&state.db)
    .await?;

    let (total_users, mfa_users): (i64, i64) = sqlx::query_as(
        r#"
        SELECT
            (SELECT COUNT(*) FROM users WHERE status <> 'pending_deletion'),
            (SELECT COUNT(DISTINCT m.user_id) FROM mfa_methods m
                JOIN users u ON u.id = m.user_id AND u.status <> 'pending_deletion'
                WHERE m.enabled_at IS NOT NULL AND m.disabled_at IS NULL)
        "#,
    )
    .fetch_one(&state.db)
    .await?;

    let (signups, ok, failed) = series.iter().fold((0, 0, 0), |acc, row| {
        (acc.0 + row.1, acc.1 + row.2, acc.2 + row.3)
    });
    let pairs = |rows: Vec<(String, i64)>| {
        rows.into_iter()
            .map(|(key, count)| json!({"key": key, "count": count}))
            .collect::<Vec<_>>()
    };
    let value = json!({
        "days": days,
        "totals": {
            "signups": signups,
            "logins_success": ok,
            "logins_failed": failed,
            "users": total_users,
            "mfa_users": mfa_users,
        },
        "active_users": { "daily": active.0, "weekly": active.1, "monthly": active.2 },
        "series": series.into_iter().map(|(day, signups, ok, failed, active)| json!({
            "day": day,
            "signups": signups,
            "logins_success": ok,
            "logins_failed": failed,
            "active_users": active,
        })).collect::<Vec<_>>(),
        "login_methods": pairs(login_methods),
        "identity_providers": pairs(providers),
        "event_types": pairs(event_types),
        "oauth_clients": pairs(clients),
    });
    let mut entries = cache.lock().await;
    entries.retain(|(d, _, _)| *d != days);
    entries.push((days, Instant::now(), value.clone()));
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

pub struct ClientQuery {
    pub q: Option<String>,
    pub status: Option<String>,
    pub client_type: Option<String>,
}

type ClientRow = (
    String,
    String,
    String,
    String,
    bool,
    bool,
    Vec<String>,
    Vec<String>,
    bool,
    DateTime<Utc>,
    i64,
    i64,
    Option<DateTime<Utc>>,
);

/// Registered OAuth clients with usage counts. Never exposes `secret_hash`.
async fn client_rows(
    state: &AppState,
    id: Option<&str>,
    query: &ClientQuery,
) -> AppResult<Vec<ClientRow>> {
    let like = query
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            format!(
                "%{}%",
                value
                    .replace('\\', "\\\\")
                    .replace('%', "\\%")
                    .replace('_', "\\_")
            )
        });
    let rows = sqlx::query_as(
        r#"
        SELECT c.id, c.name, c.client_type, c.status, c.first_party, c.require_pkce,
               c.allowed_scopes, c.redirect_uris, c.secret_hash IS NOT NULL, c.created_at,
               (SELECT COUNT(*) FROM oauth_consents oc WHERE oc.client_id = c.id),
               (SELECT COUNT(*) FROM oauth_access_tokens t
                   WHERE t.client_id = c.id AND t.revoked_at IS NULL AND t.expires_at > now()),
               (SELECT MAX(occurred_at) FROM security_events s
                   WHERE s.client_id = c.id AND s.event_type = 'OAUTH_AUTHORIZED')
        FROM oauth_clients c
        WHERE ($1::text IS NULL OR c.id = $1)
          AND ($2::text IS NULL OR c.id ILIKE $2 ESCAPE '\' OR c.name ILIKE $2 ESCAPE '\')
          AND ($3::text IS NULL OR c.status = $3)
          AND ($4::text IS NULL OR c.client_type = $4)
        ORDER BY c.first_party DESC, c.name, c.id
        "#,
    )
    .bind(id)
    .bind(like)
    .bind(query.status.as_deref())
    .bind(query.client_type.as_deref())
    .fetch_all(&state.db)
    .await?;
    Ok(rows)
}

fn client_json(row: ClientRow) -> Value {
    json!({
        "id": row.0,
        "name": row.1,
        "client_type": row.2,
        "status": row.3,
        "first_party": row.4,
        "require_pkce": row.5,
        "allowed_scopes": row.6,
        "redirect_uris": row.7,
        "has_secret": row.8,
        "created_at": row.9,
        "authorized_users": row.10,
        "active_tokens": row.11,
        "last_authorized_at": row.12,
    })
}

pub async fn list_clients(state: &AppState, query: ClientQuery) -> AppResult<Value> {
    let items = client_rows(state, None, &query)
        .await?
        .into_iter()
        .map(client_json)
        .collect::<Vec<_>>();
    Ok(json!({"items": items}))
}

pub async fn client_detail(state: &AppState, client_id: &str) -> AppResult<Value> {
    let all = ClientQuery {
        q: None,
        status: None,
        client_type: None,
    };
    let Some(row) = client_rows(state, Some(client_id), &all)
        .await?
        .into_iter()
        .next()
    else {
        return Err(AppError::NotFound);
    };
    let recent: Vec<(Uuid, Option<String>, Vec<String>, DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT oc.user_id, e.email, oc.scopes, oc.granted_at
        FROM oauth_consents oc
        LEFT JOIN user_emails e ON e.user_id = oc.user_id AND e.is_primary
        WHERE oc.client_id = $1
        ORDER BY oc.granted_at DESC
        LIMIT 20
        "#,
    )
    .bind(client_id)
    .fetch_all(&state.db)
    .await?;
    let mut value = client_json(row);
    value["recent_consents"] = recent
        .into_iter()
        .map(|(user_id, email, scopes, granted_at)| {
            json!({"user_id": user_id, "email": email, "scopes": scopes, "granted_at": granted_at})
        })
        .collect::<Vec<_>>()
        .into();
    Ok(value)
}
