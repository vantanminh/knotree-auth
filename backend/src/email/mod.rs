pub mod templates;

use crate::error::{AppError, AppResult};
use crate::state::AppState;
use async_trait::async_trait;
use chrono::Utc;
use serde::Deserialize;
use templates::RenderedEmail;
use uuid::Uuid;

#[async_trait]
pub trait EmailProvider: Send + Sync {
    fn name(&self) -> &'static str;
    async fn deliver(&self, message: &RenderedEmail, to: &str) -> AppResult<Option<String>>;
}

pub struct OutboxProvider;

#[async_trait]
impl EmailProvider for OutboxProvider {
    fn name(&self) -> &'static str {
        "outbox"
    }

    async fn deliver(&self, _message: &RenderedEmail, _to: &str) -> AppResult<Option<String>> {
        Ok(Some(format!("outbox_{}", Uuid::now_v7())))
    }
}

pub struct ResendProvider {
    api_key: String,
    from: String,
    http: reqwest::Client,
}

impl ResendProvider {
    pub fn new(api_key: String, from: String, http: reqwest::Client) -> Self {
        Self {
            api_key,
            from,
            http,
        }
    }
}

#[derive(Deserialize)]
struct ResendResponse {
    id: Option<String>,
}

#[async_trait]
impl EmailProvider for ResendProvider {
    fn name(&self) -> &'static str {
        "resend"
    }

    async fn deliver(&self, message: &RenderedEmail, to: &str) -> AppResult<Option<String>> {
        let response = self
            .http
            .post("https://api.resend.com/emails")
            .bearer_auth(&self.api_key)
            .json(&serde_json::json!({
                "from": self.from,
                "to": [to],
                "subject": message.subject,
                "html": message.html,
                "text": message.text,
            }))
            .send()
            .await
            .map_err(AppError::internal)?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            tracing::error!(status = %status, "email provider rejected message");
            let _ = body;
            return Err(AppError::EmailDelivery);
        }
        let parsed: ResendResponse = response.json().await.map_err(AppError::internal)?;
        Ok(parsed.id)
    }
}

pub async fn enqueue_and_send(
    state: &AppState,
    to: &str,
    message: RenderedEmail,
) -> AppResult<Uuid> {
    let id = Uuid::now_v7();
    sqlx::query(
        r#"
        INSERT INTO email_messages (
            id, to_address, subject, template, text_body, html_body, status, provider, created_at, attempt_count
        ) VALUES ($1, $2, $3, $4, $5, $6, 'queued', $7, $8, 0)
        "#,
    )
    .bind(id)
    .bind(to)
    .bind(&message.subject)
    .bind(message.template)
    .bind(&message.text)
    .bind(&message.html)
    .bind(state.email.name())
    .bind(Utc::now())
    .execute(&state.db)
    .await?;

    match state.email.deliver(&message, to).await {
        Ok(provider_id) => {
            sqlx::query(
                r#"
                UPDATE email_messages
                SET status = 'sent', sent_at = $2, provider_message_id = $3, attempt_count = attempt_count + 1
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(Utc::now())
            .bind(provider_id)
            .execute(&state.db)
            .await?;
            tracing::info!(template = message.template, message_id = %id, "email sent");
        }
        Err(err) => {
            sqlx::query(
                r#"
                UPDATE email_messages
                SET status = 'failed', error = $2, attempt_count = attempt_count + 1
                WHERE id = $1
                "#,
            )
            .bind(id)
            .bind(err.to_string())
            .execute(&state.db)
            .await?;
            tracing::error!(template = message.template, message_id = %id, "email send failed");
            return Err(err);
        }
    }
    Ok(id)
}

pub async fn retry_failed(state: &AppState) -> AppResult<()> {
    let rows: Vec<(Uuid, String, String, String, String, String)> = sqlx::query_as(
        r#"
        SELECT id, to_address, subject, template, text_body, html_body
        FROM email_messages
        WHERE status = 'failed' AND attempt_count < 5 AND created_at > now() - interval '1 day'
        ORDER BY created_at
        LIMIT 20
        "#,
    )
    .fetch_all(&state.db)
    .await?;
    for (id, to, subject, template, text_body, html_body) in rows {
        let message = RenderedEmail {
            subject,
            text: text_body,
            html: html_body,
            template: template_static(&template),
        };
        match state.email.deliver(&message, &to).await {
            Ok(provider_id) => {
                sqlx::query(
                    "UPDATE email_messages SET status = 'sent', sent_at = $2, provider_message_id = $3, attempt_count = attempt_count + 1, error = NULL WHERE id = $1",
                )
                .bind(id)
                .bind(Utc::now())
                .bind(provider_id)
                .execute(&state.db)
                .await?;
            }
            Err(err) => {
                sqlx::query(
                    "UPDATE email_messages SET attempt_count = attempt_count + 1, error = $2 WHERE id = $1",
                )
                .bind(id)
                .bind(err.to_string())
                .execute(&state.db)
                .await?;
            }
        }
    }
    Ok(())
}

fn template_static(name: &str) -> &'static str {
    match name {
        "verify-email" => "verify-email",
        "mfa-code" => "mfa-code",
        "reset-password" => "reset-password",
        "new-login" => "new-login",
        "security-alert" => "security-alert",
        _ => "security-alert",
    }
}
