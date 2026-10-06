use crate::admin::{self, ClientQuery, LogFilters, UserQuery};
use crate::error::AppError;
use crate::http::extract::{require_recent_auth, AdminSession, Csrf, Meta};
use crate::state::AppState;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

pub async fn stats(
    State(state): State<AppState>,
    _admin: AdminSession,
) -> Result<Json<Value>, AppError> {
    Ok(Json(admin::stats(&state).await?))
}

#[derive(Deserialize)]
pub struct AnalyticsQuery {
    days: Option<i64>,
}

pub async fn analytics(
    State(state): State<AppState>,
    _admin: AdminSession,
    Query(query): Query<AnalyticsQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        admin::analytics(&state, admin::analytics_days(query.days)).await?,
    ))
}

#[derive(Deserialize)]
pub struct UserListQuery {
    q: Option<String>,
    status: Option<String>,
    verified: Option<bool>,
    mfa: Option<bool>,
    sort: Option<String>,
    order: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
}

pub async fn users(
    State(state): State<AppState>,
    _admin: AdminSession,
    Query(query): Query<UserListQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        admin::list_users(
            &state,
            UserQuery {
                q: query.q,
                status: query.status,
                verified: query.verified,
                mfa: query.mfa,
                sort: query.sort.unwrap_or_else(|| "created_at".into()),
                order: query.order.unwrap_or_else(|| "desc".into()),
                limit: query.limit.unwrap_or(25),
                offset: query.offset.unwrap_or(0),
            },
        )
        .await?,
    ))
}

pub async fn user(
    State(state): State<AppState>,
    _admin: AdminSession,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(admin::user_detail(&state, id).await?))
}

pub async fn disable_user(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    admin::set_status(&state, admin.0.session.user_id, id, "disabled", &meta).await?;
    Ok(Json(serde_json::json!({"status": "disabled"})))
}

pub async fn enable_user(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    admin::set_status(&state, admin.0.session.user_id, id, "active", &meta).await?;
    Ok(Json(serde_json::json!({"status": "active"})))
}

pub async fn revoke_sessions(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    admin::revoke_user_sessions(&state, admin.0.session.user_id, id, &meta).await?;
    Ok(Json(serde_json::json!({"status": "revoked"})))
}

pub async fn force_reset(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    admin::force_password_reset(&state, admin.0.session.user_id, id, &meta).await?;
    Ok(Json(serde_json::json!({"status": "reset_sent"})))
}

#[derive(Deserialize)]
pub struct LogQuery {
    event_type: Option<String>,
    user_id: Option<Uuid>,
    result: Option<String>,
    request_id: Option<String>,
    from: Option<DateTime<Utc>>,
    to: Option<DateTime<Utc>>,
    limit: Option<i64>,
    offset: Option<i64>,
}

pub async fn security_events(
    State(state): State<AppState>,
    _admin: AdminSession,
    Query(query): Query<LogQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        admin::security_log(
            &state,
            LogFilters {
                event_type: query.event_type,
                user_id: query.user_id,
                result: query.result,
                request_id: query.request_id,
                from: query.from,
                to: query.to,
                limit: query.limit.unwrap_or(50),
                offset: query.offset.unwrap_or(0),
            },
        )
        .await?,
    ))
}

pub async fn logs(
    State(state): State<AppState>,
    _admin: AdminSession,
    Query(query): Query<LogQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        admin::email_log(&state, query.limit.unwrap_or(50), query.offset.unwrap_or(0)).await?,
    ))
}

#[derive(Deserialize)]
pub struct ClientListQuery {
    q: Option<String>,
    status: Option<String>,
    #[serde(rename = "type")]
    client_type: Option<String>,
}

pub async fn clients(
    State(state): State<AppState>,
    _admin: AdminSession,
    Query(query): Query<ClientListQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        admin::list_clients(
            &state,
            ClientQuery {
                q: query.q,
                status: query.status,
                client_type: query.client_type,
            },
        )
        .await?,
    ))
}

pub async fn client(
    State(state): State<AppState>,
    _admin: AdminSession,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(admin::client_detail(&state, &id).await?))
}

pub async fn create_client(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<admin::clients::ClientInput>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    Ok(Json(
        admin::clients::create_client(&state, admin.0.session.user_id, body, &meta).await?,
    ))
}

pub async fn update_client(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<String>,
    Json(body): Json<admin::clients::ClientInput>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    Ok(Json(
        admin::clients::update_client(&state, admin.0.session.user_id, &id, body, &meta).await?,
    ))
}

pub async fn rotate_client_secret(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    Ok(Json(
        admin::clients::rotate_secret(&state, admin.0.session.user_id, &id, &meta).await?,
    ))
}

pub async fn upload_client_logo(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    let mut bytes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|_| AppError::Validation("Upload a PNG, JPEG or WebP image."))?
    {
        if field.name() == Some("logo") {
            let data = field
                .bytes()
                .await
                .map_err(|_| AppError::Validation("The logo must be 1 MB or smaller."))?;
            bytes = Some(data);
            break;
        }
    }
    let Some(bytes) = bytes else {
        return Err(AppError::Validation("Upload a PNG, JPEG or WebP image."));
    };
    Ok(Json(
        admin::clients::set_logo(&state, admin.0.session.user_id, &id, &bytes, &meta).await?,
    ))
}

pub async fn delete_client_logo(
    State(state): State<AppState>,
    admin: AdminSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<String>,
) -> Result<Json<Value>, AppError> {
    require_recent_auth(&state, &admin.0.session)?;
    admin::clients::remove_logo(&state, admin.0.session.user_id, &id, &meta).await?;
    Ok(Json(serde_json::json!({"status": "removed"})))
}

pub async fn client_logo(
    State(state): State<AppState>,
    Path(file): Path<String>,
) -> Result<impl IntoResponse, AppError> {
    let bytes = admin::clients::logo_file(&state, &file).await?;
    Ok((
        [
            (header::CONTENT_TYPE, "image/png"),
            (header::CACHE_CONTROL, "public, max-age=31536000, immutable"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        bytes,
    ))
}
