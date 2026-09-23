mod account;
mod admin;
mod auth;
mod oauth;

use crate::error::AppError;
use crate::observability;
use crate::state::AppState;
use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde_json::json;

pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/ready", get(ready))
        .route("/metrics", get(metrics))
        .route("/.well-known/openid-configuration", get(oauth::discovery))
        .route("/.well-known/jwks.json", get(oauth::jwks))
        .route("/oauth/authorize", get(oauth::authorize))
        .route("/oauth/token", post(oauth::token))
        .route("/oauth/revoke", post(oauth::revoke))
        .route("/oauth/introspect", post(oauth::introspect))
        .route(
            "/oauth/userinfo",
            get(oauth::userinfo).post(oauth::userinfo),
        )
}

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .route("/api/v1/auth/csrf", get(auth::csrf))
        .route("/api/v1/auth/register", post(auth::register))
        .route("/api/v1/auth/login", post(auth::login))
        .route("/api/v1/auth/logout", post(auth::logout))
        .route("/api/v1/auth/step-up", post(auth::step_up))
        .route("/api/v1/auth/mfa/verify", post(auth::verify_mfa))
        .route("/api/v1/auth/mfa/email/send", post(auth::send_email_mfa))
        .route("/api/v1/auth/email/verify", post(auth::verify_email))
        .route("/api/v1/auth/email/resend", post(auth::resend_email))
        .route("/api/v1/auth/password/forgot", post(auth::forgot_password))
        .route("/api/v1/auth/password/reset", post(auth::reset_password))
        .route("/api/v1/auth/password/change", post(auth::change_password))
        .route(
            "/api/v1/auth/social/{provider}/start",
            get(auth::social_start),
        )
        .route(
            "/api/v1/auth/social/{provider}/callback",
            get(auth::social_callback),
        )
        .route("/api/v1/oauth/context", get(oauth::context))
        .route("/api/v1/oauth/consent", post(oauth::consent))
        .route("/api/v1/me", get(account::me))
        .route(
            "/api/v1/me/profile",
            axum::routing::patch(account::update_profile),
        )
        .route("/api/v1/me/email", post(account::change_email))
        .route("/api/v1/me/security", get(account::security))
        .route("/api/v1/me/security-events", get(account::security_events))
        .route("/api/v1/me/sessions", get(account::sessions))
        .route(
            "/api/v1/me/sessions/{id}",
            axum::routing::delete(account::revoke_session),
        )
        .route(
            "/api/v1/me/sessions/revoke-others",
            post(account::revoke_others),
        )
        .route("/api/v1/me/deletion", post(account::delete_account))
        .route(
            "/api/v1/me/identities/{provider}",
            axum::routing::delete(account::unlink),
        )
        .route("/api/v1/me/mfa/totp/setup", post(account::totp_setup))
        .route("/api/v1/me/mfa/totp/confirm", post(account::totp_confirm))
        .route("/api/v1/me/mfa/totp/disable", post(account::totp_disable))
        .route("/api/v1/me/mfa/email", post(account::email_mfa))
        .route(
            "/api/v1/me/mfa/recovery-codes",
            get(account::recovery_count).post(account::recovery_regenerate),
        )
        .route("/api/v1/admin/stats", get(admin::stats))
        .route("/api/v1/admin/users", get(admin::users))
        .route("/api/v1/admin/users/{id}", get(admin::user))
        .route(
            "/api/v1/admin/users/{id}/disable",
            post(admin::disable_user),
        )
        .route("/api/v1/admin/users/{id}/enable", post(admin::enable_user))
        .route(
            "/api/v1/admin/users/{id}/revoke-sessions",
            post(admin::revoke_sessions),
        )
        .route(
            "/api/v1/admin/users/{id}/force-password-reset",
            post(admin::force_reset),
        )
        .route("/api/v1/admin/security-events", get(admin::security_events))
        .route("/api/v1/admin/logs", get(admin::logs))
        .route("/api/v1/dev/mailbox", get(auth::dev_mailbox))
}

async fn health() -> Json<serde_json::Value> {
    Json(json!({"status": "ok", "service": "knotree-accounts"}))
}

async fn ready(State(state): State<AppState>) -> Result<Json<serde_json::Value>, AppError> {
    sqlx::query("SELECT 1").execute(&state.db).await?;
    let idle = state.db.num_idle();
    let size = state.db.size();
    metrics::gauge!("db_pool_idle").set(idle as f64);
    metrics::gauge!("db_pool_size").set(size as f64);
    Ok(Json(json!({"status": "ready"})))
}

async fn metrics(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<([(axum::http::HeaderName, &'static str); 1], String), AppError> {
    if let Some(expected) = state.config.metrics_token.as_deref() {
        let presented = headers
            .get(axum::http::header::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "));
        if presented != Some(expected) {
            return Err(AppError::NotFound);
        }
    } else if state.config.env.is_production() {
        return Err(AppError::NotFound);
    }
    Ok((
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4",
        )],
        observability::render(),
    ))
}
