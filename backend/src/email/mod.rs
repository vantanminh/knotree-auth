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
    async fn deliver(
        &self,
        message: &RenderedEmail,
        to: &str,
    ) -> Result<Option<String>, EmailFailure>;
}

#[derive(Debug)]
pub struct EmailFailure {
    retryable: bool,
    summary: String,
    code: Option<i64>,
    provider_message: Option<String>,
}

impl EmailFailure {
    fn retryable(summary: impl Into<String>) -> Self {
        Self {
            retryable: true,
            summary: summary.into(),
            code: None,
            provider_message: None,
        }
    }

    fn permanent(summary: impl Into<String>) -> Self {
        Self {
            retryable: false,
            summary: summary.into(),
            code: None,
            provider_message: None,
        }
    }

    fn stored_error(&self) -> String {
        let summary = truncate_chars(&self.summary, 200);
        if self.retryable {
            summary
        } else {
            format!("permanent {summary}")
        }
    }
}

impl From<EmailFailure> for AppError {
    fn from(_: EmailFailure) -> Self {
        AppError::EmailDelivery
    }
}

pub struct OutboxProvider;

#[async_trait]
impl EmailProvider for OutboxProvider {
    fn name(&self) -> &'static str {
        "outbox"
    }

    async fn deliver(
        &self,
        _message: &RenderedEmail,
        _to: &str,
    ) -> Result<Option<String>, EmailFailure> {
        Ok(Some(format!("outbox_{}", Uuid::now_v7())))
    }
}

pub struct CloudflareEmailProvider {
    account_id: String,
    api_token: String,
    from: String,
    from_name: Option<String>,
    http: reqwest::Client,
}

impl CloudflareEmailProvider {
    pub fn new(
        account_id: String,
        api_token: String,
        from: String,
        from_name: Option<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            account_id,
            api_token,
            from,
            from_name,
            http,
        }
    }
}

#[derive(Deserialize)]
struct CloudflareSendResponse {
    success: bool,
    result: Option<CloudflareSendResult>,
}

#[derive(Default, Deserialize)]
struct CloudflareSendResult {
    message_id: Option<String>,
    #[serde(default)]
    delivered: Vec<String>,
    #[serde(default)]
    queued: Vec<String>,
    #[serde(default)]
    permanent_bounces: Vec<String>,
    #[serde(default)]
    suppressed_recipients: Vec<String>,
}

#[derive(Deserialize)]
struct CloudflareErrorBody {
    #[serde(default)]
    errors: Vec<CloudflareApiError>,
}

#[derive(Deserialize)]
struct CloudflareApiError {
    code: Option<i64>,
    message: Option<String>,
}

#[async_trait]
impl EmailProvider for CloudflareEmailProvider {
    fn name(&self) -> &'static str {
        "cloudflare"
    }

    async fn deliver(
        &self,
        message: &RenderedEmail,
        to: &str,
    ) -> Result<Option<String>, EmailFailure> {
        let payload = cloudflare_payload(&self.from, self.from_name.as_deref(), to, message);
        let body = serde_json::to_vec(&payload).map_err(|_| {
            tracing::error!("email payload could not be encoded");
            EmailFailure::permanent("encode")
        })?;
        let response = self
            .http
            .post(format!(
                "https://api.cloudflare.com/client/v4/accounts/{}/email/sending/send",
                self.account_id
            ))
            .bearer_auth(&self.api_token)
            .header(reqwest::header::CONTENT_TYPE, "application/json")
            .header(reqwest::header::CONTENT_LENGTH, body.len().to_string())
            .body(body)
            .send()
            .await
            .map_err(|err| {
                tracing::error!(error = %err, "email provider request failed");
                EmailFailure::retryable("transport")
            })?;
        let status = response.status();
        if !status.is_success() {
            let failure = rejection_from_body(status, &response.text().await.unwrap_or_default());
            tracing::error!(
                status = %status,
                provider_code = failure.code,
                provider_message = failure.provider_message.as_deref().unwrap_or(""),
                retryable = failure.retryable,
                "email provider rejected message"
            );
            return Err(failure);
        }
        let parsed: CloudflareSendResponse = response.json().await.map_err(|_| {
            tracing::error!(status = %status, "email provider returned an invalid response");
            EmailFailure::retryable(format!("{} invalid_response", status.as_u16()))
        })?;
        let Some(result) = parsed.result.filter(|_| parsed.success) else {
            tracing::error!(status = %status, "email provider did not accept message");
            return Err(EmailFailure::retryable(format!(
                "{} not_accepted",
                status.as_u16()
            )));
        };
        if result
            .suppressed_recipients
            .iter()
            .any(|recipient| recipient.eq_ignore_ascii_case(to))
        {
            tracing::error!(status = %status, "email provider suppressed recipient");
            return Err(EmailFailure::permanent(format!(
                "{} email.sending.error.recipient_suppressed",
                status.as_u16()
            )));
        }
        let accepted = result
            .delivered
            .iter()
            .chain(result.queued.iter())
            .any(|recipient| recipient.eq_ignore_ascii_case(to));
        if accepted {
            return Ok(result.message_id);
        }

        if result
            .permanent_bounces
            .iter()
            .any(|recipient| recipient.eq_ignore_ascii_case(to))
        {
            tracing::error!(status = %status, "email provider reported a permanent bounce");
            return Err(EmailFailure::permanent(format!(
                "{} permanent_bounce",
                status.as_u16()
            )));
        }
        tracing::error!(status = %status, "email provider did not accept recipient");
        Err(EmailFailure::retryable(format!(
            "{} recipient_not_accepted",
            status.as_u16()
        )))
    }
}

fn cloudflare_payload(
    from: &str,
    from_name: Option<&str>,
    to: &str,
    message: &RenderedEmail,
) -> serde_json::Value {
    let from = match from_name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => serde_json::json!({ "address": from, "name": name }),
        None => serde_json::json!(from),
    };
    // The Email Sending API example sends recipients as a list.
    serde_json::json!({
        "from": from,
        "to": [to],
        "subject": message.subject,
        "html": message.html,
        "text": message.text,
    })
}

fn rejection_from_body(status: reqwest::StatusCode, body: &str) -> EmailFailure {
    let parsed: CloudflareErrorBody =
        serde_json::from_str(body).unwrap_or(CloudflareErrorBody { errors: Vec::new() });
    let error = parsed.errors.first();
    let code = error.and_then(|item| item.code);
    let provider_message = error
        .and_then(|item| item.message.as_deref())
        .and_then(safe_provider_message)
        .map(str::to_string);
    let retryable = status.as_u16() == 429 || status.is_server_error();
    let status_code = status.as_u16();
    let summary = match (code, provider_message.as_deref()) {
        (Some(code), Some(message)) => format!("{status_code} {code} {message}"),
        (Some(code), None) => format!("{status_code} {code}"),
        (None, Some(message)) => format!("{status_code} {message}"),
        (None, None) => status_code.to_string(),
    };
    EmailFailure {
        retryable,
        summary,
        code,
        provider_message,
    }
}

fn safe_provider_message(message: &str) -> Option<&str> {
    let machine_readable = !message.is_empty()
        && message.len() <= 80
        && message
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | ':'));
    machine_readable.then_some(message)
}

fn truncate_chars(value: &str, max: usize) -> String {
    value.chars().take(max).collect()
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
            .bind(err.stored_error())
            .execute(&state.db)
            .await?;
            tracing::error!(template = message.template, message_id = %id, "email send failed");
            return Err(err.into());
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
          AND (error IS NULL OR error NOT LIKE 'permanent %')
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
                .bind(err.stored_error())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(subject: &str) -> RenderedEmail {
        RenderedEmail {
            subject: subject.into(),
            text: "Reset link".into(),
            html: "<p>Reset link</p>".into(),
            template: "reset-password",
        }
    }

    #[test]
    fn cloudflare_payload_names_the_sender_and_lists_the_recipient() {
        let body = cloudflare_payload(
            "accounts@knotree.com",
            Some(" Knotree Accounts "),
            "user@example.com",
            &sample("Reset your password"),
        );
        assert_eq!(body["to"], serde_json::json!(["user@example.com"]));
        assert_eq!(body["from"]["address"], "accounts@knotree.com");
        assert_eq!(body["from"]["name"], "Knotree Accounts");
        assert_eq!(body["subject"], "Reset your password");
        assert!(body.get("cc").is_none());
    }

    #[test]
    fn cloudflare_payload_omits_an_empty_display_name() {
        let body = cloudflare_payload(
            "accounts@knotree.com",
            Some("  "),
            "user@example.com",
            &sample("Reset your password"),
        );
        assert_eq!(body["from"], "accounts@knotree.com");
    }

    #[test]
    fn provider_validation_errors_are_permanent_and_do_not_keep_addresses() {
        let failure = rejection_from_body(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"success":false,"errors":[{"code":10001,"message":"email.sending.error.invalid_request_schema"}],"messages":[],"result":null}"#,
        );
        assert!(!failure.retryable);
        assert_eq!(
            failure.stored_error(),
            "permanent 400 10001 email.sending.error.invalid_request_schema"
        );

        let leaked = rejection_from_body(
            reqwest::StatusCode::BAD_REQUEST,
            r#"{"errors":[{"code":10202,"message":"rejected user@example.com"}]}"#,
        );
        assert!(!leaked.stored_error().contains('@'));
        assert!(!leaked.stored_error().contains("user"));

        let retryable = rejection_from_body(reqwest::StatusCode::TOO_MANY_REQUESTS, "{}");
        assert!(retryable.retryable);
        assert!(!retryable.stored_error().starts_with("permanent "));

        let server = rejection_from_body(reqwest::StatusCode::BAD_GATEWAY, "");
        assert!(server.retryable);
    }
}
