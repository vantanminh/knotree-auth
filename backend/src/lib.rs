#![allow(clippy::type_complexity, clippy::too_many_arguments)]

mod admin;
pub mod auth;
pub mod config;
mod email;
mod error;
mod http;
mod i18n;
mod oauth;
mod observability;
mod security;
mod state;

pub use error::{AppError, AppResult};
pub use http::router;
pub use state::{connect, AppState};

pub fn observability_init() {
    observability::init();
}

pub async fn email_retry(state: &AppState) -> AppResult<()> {
    email::retry_failed(state).await
}

#[cfg(feature = "test-util")]
pub use config::for_tests;
