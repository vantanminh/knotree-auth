use crate::auth::{self, LoginResult};
use crate::error::AppError;
use crate::http::extract::{self, issue_csrf, AuthSession, Csrf, Meta, OptionalSession};
use crate::state::AppState;
use axum::extract::{Path, Query, State};
use axum::Json;
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

#[derive(Deserialize)]
pub struct RegisterBody {
    username: String,
    email: String,
    password: String,
    password_confirm: String,
    return_to: Option<String>,
}

#[derive(Deserialize)]
pub struct LoginBody {
    /// A username or any verified email of the account.
    #[serde(alias = "email")]
    identifier: String,
    password: String,
}

#[derive(Deserialize)]
pub struct MfaBody {
    mfa_token: String,
    method: String,
    code: String,
}

#[derive(Deserialize)]
pub struct TokenBody {
    token: String,
}

#[derive(Deserialize)]
pub struct EmailBody {
    email: String,
}

#[derive(Deserialize)]
pub struct ResetBody {
    token: String,
    password: String,
}

#[derive(Deserialize)]
pub struct ChangePasswordBody {
    current_password: String,
    new_password: String,
}

#[derive(Deserialize)]
pub struct StepUpBody {
    password: String,
    method: Option<String>,
    code: Option<String>,
}

#[derive(Deserialize)]
pub struct SocialStartQuery {
    mode: Option<String>,
    return_to: Option<String>,
}

pub async fn csrf(
    State(state): State<AppState>,
    jar: CookieJar,
) -> Result<(CookieJar, Json<Value>), AppError> {
    let (jar, token) = issue_csrf(&state.config, jar)?;
    Ok((jar, Json(json!({"csrf_token": token}))))
}

pub async fn register(
    State(state): State<AppState>,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<RegisterBody>,
) -> Result<Json<Value>, AppError> {
    let user_id = auth::register(
        &state,
        auth::RegisterInput {
            username: body.username,
            email: body.email,
            password: body.password,
            password_confirm: body.password_confirm,
            return_to: body.return_to,
        },
        &meta,
    )
    .await?;
    let _ = auth::bootstrap_admin(&state).await;
    Ok(Json(json!({
        "status": "created",
        "user_id": user_id,
        "message": "Account created. Check your email to verify it."
    })))
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<LoginBody>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    match auth::login(&state, &body.identifier, &body.password, &meta).await? {
        LoginResult::Session(issued) => {
            let jar = jar.add(extract::build_cookie(
                &state.config,
                extract::session_cookie_name(&state.config),
                &issued.token,
                true,
                (issued.session.expires_at - chrono::Utc::now())
                    .num_seconds()
                    .max(60),
            ));
            Ok((jar, Json(json!({"status": "authenticated"}))))
        }
        LoginResult::Mfa {
            token,
            methods,
            masked_email,
        } => Ok((
            jar,
            Json(json!({
                "status": "mfa_required",
                "mfa_token": token,
                "methods": methods,
                "masked_email": masked_email,
            })),
        )),
    }
}

pub async fn verify_mfa(
    State(state): State<AppState>,
    jar: CookieJar,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<MfaBody>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    let issued =
        auth::verify_login(&state, &body.mfa_token, &body.method, &body.code, &meta).await?;
    let jar = jar.add(extract::build_cookie(
        &state.config,
        extract::session_cookie_name(&state.config),
        &issued.token,
        true,
        (issued.session.expires_at - chrono::Utc::now())
            .num_seconds()
            .max(60),
    ));
    Ok((jar, Json(json!({"status": "authenticated"}))))
}

pub async fn send_email_mfa(
    State(state): State<AppState>,
    Csrf: Csrf,
    Json(body): Json<Value>,
) -> Result<Json<Value>, AppError> {
    let token = body.get("mfa_token").and_then(|v| v.as_str()).unwrap_or("");
    let masked = auth::send_login_email_otp(&state, token).await?;
    Ok(Json(json!({"status": "sent", "masked_email": masked})))
}

pub async fn logout(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
) -> Result<(CookieJar, Json<Value>), AppError> {
    auth::revoke_owned(&state, auth.session.user_id, auth.session.id, &meta).await?;
    let jar = auth.jar.add(extract::clear_cookie(
        &state.config,
        extract::session_cookie_name(&state.config),
    ));
    Ok((jar, Json(json!({"status": "signed_out"}))))
}

pub async fn step_up(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<StepUpBody>,
) -> Result<(CookieJar, Json<Value>), AppError> {
    if !auth::step_up_password(&state, &auth.session, &body.password).await? {
        return Err(AppError::InvalidCredentials);
    }
    let methods = auth::enabled_mfa_methods(&state, auth.session.user_id).await?;
    if methods.iter().any(|method| method == "totp") {
        let method = body.method.as_deref().unwrap_or("totp");
        let code = body.code.as_deref().unwrap_or("");
        if !auth::verify_second_factor(&state, auth.session.user_id, method, code).await? {
            return Err(AppError::Validation("That code is not valid."));
        }
    }
    let issued = auth::rotate(&state, &auth.session, &meta).await?;
    let jar = auth.jar.add(extract::build_cookie(
        &state.config,
        extract::session_cookie_name(&state.config),
        &issued.token,
        true,
        (issued.session.expires_at - chrono::Utc::now())
            .num_seconds()
            .max(60),
    ));
    Ok((jar, Json(json!({"status": "confirmed"}))))
}

pub async fn verify_email(
    State(state): State<AppState>,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<TokenBody>,
) -> Result<Json<Value>, AppError> {
    match auth::verify_email(&state, &body.token, &meta).await {
        Ok(return_to) => Ok(Json(json!({"status": "verified", "return_to": return_to}))),
        Err(AppError::Gone(_)) => {
            auth::confirm_added_email(&state, &body.token, &meta).await?;
            Ok(Json(json!({"status": "email_added"})))
        }
        Err(err) => Err(err),
    }
}

pub async fn resend_email(
    State(state): State<AppState>,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<EmailBody>,
) -> Result<Json<Value>, AppError> {
    auth::resend_verification(&state, &body.email, &meta).await?;
    Ok(Json(
        json!({"message": "If an unverified account exists for this email, we sent a new link."}),
    ))
}

#[derive(Deserialize)]
pub struct ForgotBody {
    #[serde(alias = "email")]
    identifier: String,
}

pub async fn forgot_password(
    State(state): State<AppState>,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<ForgotBody>,
) -> Result<Json<Value>, AppError> {
    auth::request_reset(&state, &body.identifier, &meta).await?;
    Ok(Json(
        json!({"message": "If an account exists for this email, we've sent password reset instructions."}),
    ))
}

pub async fn reset_password(
    State(state): State<AppState>,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<ResetBody>,
) -> Result<Json<Value>, AppError> {
    auth::reset_password(&state, &body.token, &body.password, &meta).await?;
    Ok(Json(json!({"status": "password_reset"})))
}

pub async fn change_password(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<ChangePasswordBody>,
) -> Result<Json<Value>, AppError> {
    auth::change_password(
        &state,
        &auth.session,
        &body.current_password,
        &body.new_password,
        &meta,
    )
    .await?;
    Ok(Json(json!({"status": "password_changed"})))
}

pub async fn social_start(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(provider): Path<String>,
    Query(query): Query<SocialStartQuery>,
    OptionalSession(parts): OptionalSession,
) -> Result<(CookieJar, axum::response::Redirect), AppError> {
    let mode = query.mode.unwrap_or_else(|| "login".into());
    let user_id = parts.as_ref().map(|auth| auth.session.user_id);
    let started = auth::start(
        &state,
        &provider,
        &mode,
        user_id,
        query.return_to.as_deref(),
    )
    .await?;
    let jar = jar.add(extract::build_cookie(
        &state.config,
        extract::social_cookie_name(&state.config),
        &started.state_token,
        true,
        600,
    ));
    Ok((
        jar,
        axum::response::Redirect::temporary(&started.authorize_url),
    ))
}

pub async fn social_callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Path(provider): Path<String>,
    Query(query): Query<HashMap<String, String>>,
    Meta(meta): Meta,
) -> Result<(CookieJar, axum::response::Redirect), AppError> {
    let code = query.get("code").map(String::as_str).unwrap_or("");
    let state_token = query.get("state").map(String::as_str).unwrap_or("");
    let cookie = jar
        .get(extract::social_cookie_name(&state.config))
        .map(|cookie| cookie.value().to_string())
        .unwrap_or_default();
    if cookie.is_empty() || cookie != state_token {
        return Err(AppError::Validation(
            "This sign-in attempt expired. Start again.",
        ));
    }
    let finished = auth::finish(&state, &provider, code, state_token, &meta).await?;
    let mut jar = jar.add(extract::clear_cookie(
        &state.config,
        extract::social_cookie_name(&state.config),
    ));
    let destination = if let Some(issued) = finished.session {
        jar = jar.add(extract::build_cookie(
            &state.config,
            extract::session_cookie_name(&state.config),
            &issued.token,
            true,
            (issued.session.expires_at - chrono::Utc::now())
                .num_seconds()
                .max(60),
        ));
        finished.return_to.unwrap_or_else(|| "/account".into())
    } else if finished.linked {
        finished
            .return_to
            .unwrap_or_else(|| "/account/connected-accounts".into())
    } else {
        "/sign-in".into()
    };
    Ok((jar, axum::response::Redirect::temporary(&destination)))
}

pub async fn dev_mailbox(
    State(state): State<AppState>,
    Query(query): Query<EmailBody>,
) -> Result<Json<Value>, AppError> {
    if !state.config.dev_mailbox || state.config.env.is_production() {
        return Err(AppError::NotFound);
    }
    let rows: Vec<(
        uuid::Uuid,
        String,
        String,
        String,
        chrono::DateTime<chrono::Utc>,
    )> = sqlx::query_as(
        r#"
        SELECT id, template, subject, text_body, created_at
        FROM email_messages
        WHERE to_address = $1
        ORDER BY created_at DESC
        LIMIT 10
        "#,
    )
    .bind(query.email.trim().to_lowercase())
    .fetch_all(&state.db)
    .await?;
    Ok(Json(json!({
        "messages": rows.into_iter().map(|row| json!({
            "id": row.0,
            "template": row.1,
            "subject": row.2,
            "text": row.3,
            "created_at": row.4,
        })).collect::<Vec<_>>()
    })))
}
