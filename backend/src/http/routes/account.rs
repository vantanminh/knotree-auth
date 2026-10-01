use crate::auth::{self, CurrentSession};
use crate::error::AppError;
use crate::http::extract::{self, AuthSession, Csrf, Meta};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

fn guard(state: &AppState, session: &CurrentSession) -> Result<(), AppError> {
    extract::require_recent_auth(state, session)
}

pub async fn me(State(state): State<AppState>, auth: AuthSession) -> Result<Json<Value>, AppError> {
    Ok(Json(auth::profile(&state, &auth.session).await?))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProfileBody {
    display_name: Option<String>,
    locale: Option<String>,
}

pub async fn update_profile(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<ProfileBody>,
) -> Result<Json<Value>, AppError> {
    auth::update_profile(
        &state,
        auth.session.user_id,
        body.display_name.as_deref(),
        body.locale.as_deref(),
        &meta,
    )
    .await?;
    Ok(Json(json!({"status": "updated"})))
}

#[derive(Deserialize)]
pub struct EmailChangeBody {
    email: String,
}

pub async fn change_email(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<EmailChangeBody>,
) -> Result<Json<Value>, AppError> {
    auth::request_email_change(&state, &auth.session, &body.email, &meta).await?;
    Ok(Json(json!({"status": "verification_sent"})))
}

pub async fn security(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<Json<Value>, AppError> {
    Ok(Json(
        auth::security_summary(&state, auth.session.user_id).await?,
    ))
}

#[derive(Deserialize)]
pub struct LimitQuery {
    limit: Option<i64>,
}

pub async fn security_events(
    State(state): State<AppState>,
    auth: AuthSession,
    Query(query): Query<LimitQuery>,
) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "items": auth::security_events(&state, auth.session.user_id, query.limit.unwrap_or(30)).await?
    })))
}

pub async fn sessions(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "items": auth::list_sessions(&state, auth.session.user_id, auth.session.id).await?
    })))
}

pub async fn revoke_session(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(id): Path<Uuid>,
) -> Result<(axum_extra::extract::CookieJar, Json<Value>), AppError> {
    auth::revoke_owned(&state, auth.session.user_id, id, &meta).await?;
    let mut jar = auth.jar;
    if id == auth.session.id {
        jar = jar.add(extract::clear_cookie(
            &state.config,
            extract::session_cookie_name(&state.config),
        ));
    }
    Ok((jar, Json(json!({"status": "revoked"}))))
}

pub async fn revoke_others(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
) -> Result<Json<Value>, AppError> {
    guard(&state, &auth.session)?;
    auth::revoke_all(
        &state.db,
        auth.session.user_id,
        Some(auth.session.id),
        "user_revoke_others",
    )
    .await?;
    let mut event = auth::NewEvent::success("SESSION_REVOKED", auth.session.user_id);
    event.metadata = serde_json::json!({"scope": "others"});
    auth::apply_meta(&mut event, &meta);
    auth::record(&state.db, event).await?;
    Ok(Json(json!({"status": "revoked"})))
}

pub async fn delete_account(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
) -> Result<(axum_extra::extract::CookieJar, Json<Value>), AppError> {
    auth::delete_account(&state, &auth.session, &meta).await?;
    let jar = auth.jar.add(extract::clear_cookie(
        &state.config,
        extract::session_cookie_name(&state.config),
    ));
    Ok((jar, Json(json!({"status": "deletion_requested"}))))
}

pub async fn unlink(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Path(provider): Path<String>,
) -> Result<Json<Value>, AppError> {
    guard(&state, &auth.session)?;
    auth::unlink(&state, auth.session.user_id, &provider, &meta).await?;
    Ok(Json(json!({"status": "disconnected"})))
}

pub async fn totp_setup(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
) -> Result<Json<Value>, AppError> {
    guard(&state, &auth.session)?;
    let setup = auth::begin_totp(&state, auth.session.user_id, &meta).await?;
    Ok(Json(json!({
        "secret": setup.secret,
        "otpauth_uri": setup.otpauth_uri,
        "qr_svg": setup.qr_svg,
    })))
}

#[derive(Deserialize)]
pub struct CodeBody {
    code: String,
    method: Option<String>,
}

pub async fn totp_confirm(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<CodeBody>,
) -> Result<Json<Value>, AppError> {
    guard(&state, &auth.session)?;
    let codes = auth::confirm_totp(&state, auth.session.user_id, &body.code, &meta).await?;
    Ok(Json(json!({"recovery_codes": codes})))
}

pub async fn totp_disable(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<CodeBody>,
) -> Result<Json<Value>, AppError> {
    guard(&state, &auth.session)?;
    auth::disable_totp(
        &state,
        auth.session.user_id,
        &body.code,
        body.method.as_deref().unwrap_or("totp"),
        &meta,
    )
    .await?;
    Ok(Json(json!({"status": "disabled"})))
}

#[derive(Deserialize)]
pub struct EmailMfaBody {
    enabled: bool,
}

pub async fn email_mfa(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<EmailMfaBody>,
) -> Result<Json<Value>, AppError> {
    guard(&state, &auth.session)?;
    auth::set_email_mfa(&state, auth.session.user_id, body.enabled, &meta).await?;
    Ok(Json(json!({"status": "updated"})))
}

pub async fn recovery_count(
    State(state): State<AppState>,
    auth: AuthSession,
) -> Result<Json<Value>, AppError> {
    Ok(Json(json!({
        "remaining": auth::recovery_remaining(&state, auth.session.user_id).await?
    })))
}

pub async fn recovery_regenerate(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
) -> Result<Json<Value>, AppError> {
    guard(&state, &auth.session)?;
    let codes = auth::regenerate_recovery(&state, auth.session.user_id, &meta).await?;
    Ok(Json(json!({"recovery_codes": codes})))
}
