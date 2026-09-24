use crate::config::AppConfig;
use crate::email::{EmailProvider, OutboxProvider, ResendProvider};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
pub struct AppState {
    pub db: PgPool,
    pub config: Arc<AppConfig>,
    pub email: Arc<dyn EmailProvider>,
    pub http: reqwest::Client,
}

pub async fn connect(config: AppConfig) -> Result<AppState, crate::error::AppError> {
    let pool = PgPoolOptions::new()
        .max_connections(20)
        .min_connections(1)
        .acquire_timeout(Duration::from_secs(5))
        .idle_timeout(Duration::from_secs(600))
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET statement_timeout = '5s'")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(&config.database_url)
        .await
        .map_err(crate::error::AppError::internal)?;

    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .map_err(crate::error::AppError::internal)?;

    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("KnotreeAccounts/0.1")
        .build()
        .map_err(crate::error::AppError::internal)?;

    let email: Arc<dyn EmailProvider> = if config.email_provider == "resend" {
        if let Some(key) = config.resend_api_key.clone() {
            Arc::new(ResendProvider::new(
                key,
                config.email_from.clone(),
                http.clone(),
            ))
        } else {
            Arc::new(OutboxProvider)
        }
    } else {
        Arc::new(OutboxProvider)
    };

    Ok(AppState {
        db: pool,
        config: Arc::new(config),
        email,
        http,
    })
}
