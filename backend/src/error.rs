use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Serialize;
use std::fmt;

tokio::task_local! {
    pub static REQUEST_ID: String;
}

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("rate limited")]
    RateLimited { retry_after_seconds: u64 },
    #[error("{0}")]
    Validation(&'static str),
    #[error("{0}")]
    Conflict(&'static str),
    #[error("unauthenticated")]
    Unauthenticated,
    #[error("{0}")]
    Forbidden(&'static str),
    #[error("not found")]
    NotFound,
    #[error("{0}")]
    Gone(&'static str),
    #[error("account disabled")]
    AccountDisabled,
    #[error("password reset required")]
    PasswordResetRequired,
    #[error("email delivery failed")]
    EmailDelivery,
    #[error("step-up required")]
    StepUpRequired,
    #[error("internal error")]
    Internal(String),
}

impl AppError {
    pub fn internal(err: impl fmt::Display) -> Self {
        tracing::error!(error = %err, "internal error");
        Self::Internal(err.to_string())
    }

    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidCredentials => "INVALID_CREDENTIALS",
            Self::RateLimited { .. } => "RATE_LIMITED",
            Self::Validation(_) => "VALIDATION_ERROR",
            Self::Conflict(_) => "CONFLICT",
            Self::Unauthenticated => "UNAUTHENTICATED",
            Self::Forbidden(_) => "FORBIDDEN",
            Self::NotFound => "NOT_FOUND",
            Self::Gone(_) => "EXPIRED",
            Self::AccountDisabled => "ACCOUNT_DISABLED",
            Self::PasswordResetRequired => "PASSWORD_RESET_REQUIRED",
            Self::EmailDelivery => "EMAIL_DELIVERY_FAILED",
            Self::StepUpRequired => "STEP_UP_REQUIRED",
            Self::Internal(_) => "INTERNAL",
        }
    }

    pub fn status(&self) -> StatusCode {
        match self {
            Self::InvalidCredentials | Self::Unauthenticated => StatusCode::UNAUTHORIZED,
            Self::RateLimited { .. } => StatusCode::TOO_MANY_REQUESTS,
            Self::Validation(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Forbidden(_)
            | Self::AccountDisabled
            | Self::PasswordResetRequired
            | Self::StepUpRequired => StatusCode::FORBIDDEN,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Gone(_) => StatusCode::GONE,
            Self::EmailDelivery => StatusCode::BAD_GATEWAY,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    pub fn startup_message(&self) -> String {
        match self {
            Self::Internal(message) => message.clone(),
            other => other.public_message(),
        }
    }

    pub fn public_message(&self) -> String {
        match self {
            Self::InvalidCredentials => "The email or password is incorrect.".into(),
            Self::RateLimited { .. } => "Too many attempts. Try again later.".into(),
            Self::Validation(msg)
            | Self::Conflict(msg)
            | Self::Forbidden(msg)
            | Self::Gone(msg) => (*msg).into(),
            Self::Unauthenticated => "Sign in to continue.".into(),
            Self::NotFound => "Not found.".into(),
            Self::AccountDisabled => "This account is disabled.".into(),
            Self::PasswordResetRequired => "Reset your password to continue.".into(),
            Self::EmailDelivery => "The email could not be sent. Try again.".into(),
            Self::StepUpRequired => "Confirm your password again to continue.".into(),
            Self::Internal(_) => "Something went wrong.".into(),
        }
    }
}

impl From<sqlx::Error> for AppError {
    fn from(value: sqlx::Error) -> Self {
        Self::internal(value)
    }
}

#[derive(Serialize)]
struct ErrorBody<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'a str,
    message: String,
    request_id: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let request_id = REQUEST_ID
            .try_with(|id| id.clone())
            .unwrap_or_else(|_| "req_unknown".into());
        let status = self.status();
        let body = Json(ErrorBody {
            error: ErrorDetail {
                code: self.code(),
                message: self.public_message(),
                request_id,
            },
        });
        let mut response = (status, body).into_response();
        if let Self::RateLimited {
            retry_after_seconds,
        } = self
        {
            if let Ok(value) = HeaderValue::from_str(&retry_after_seconds.to_string()) {
                response.headers_mut().insert("retry-after", value);
            }
        }
        response
    }
}

pub fn is_unique_violation(err: &sqlx::Error) -> bool {
    err.as_database_error()
        .and_then(|db| db.code())
        .is_some_and(|code| code == "23505")
}

pub type AppResult<T> = Result<T, AppError>;
