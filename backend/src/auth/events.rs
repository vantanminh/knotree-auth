use crate::error::AppResult;
use chrono::{DateTime, Utc};
use ipnetwork::IpNetwork;
use serde_json::{json, Value};
use sqlx::PgExecutor;
use uuid::Uuid;

pub struct NewEvent {
    pub event_type: &'static str,
    pub result: &'static str,
    pub actor_user_id: Option<Uuid>,
    pub target_user_id: Option<Uuid>,
    pub client_id: Option<String>,
    pub session_id: Option<Uuid>,
    pub request_id: Option<String>,
    pub ip: Option<IpNetwork>,
    pub user_agent: Option<String>,
    pub metadata: Value,
}

impl NewEvent {
    pub fn success(event_type: &'static str, user_id: Uuid) -> Self {
        Self {
            event_type,
            result: "success",
            actor_user_id: Some(user_id),
            target_user_id: Some(user_id),
            client_id: None,
            session_id: None,
            request_id: None,
            ip: None,
            user_agent: None,
            metadata: json!({}),
        }
    }
}

pub async fn record<'e, E>(executor: E, event: NewEvent) -> AppResult<Uuid>
where
    E: PgExecutor<'e>,
{
    let id = Uuid::now_v7();
    let occurred_at: DateTime<Utc> = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO security_events (
            id, occurred_at, event_type, result, actor_user_id, target_user_id,
            client_id, session_id, request_id, ip, user_agent, metadata
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)
        "#,
    )
    .bind(id)
    .bind(occurred_at)
    .bind(event.event_type)
    .bind(event.result)
    .bind(event.actor_user_id)
    .bind(event.target_user_id)
    .bind(event.client_id)
    .bind(event.session_id)
    .bind(event.request_id)
    .bind(event.ip)
    .bind(event.user_agent.as_deref().map(truncate_ua))
    .bind(event.metadata)
    .execute(executor)
    .await?;
    Ok(id)
}

pub fn truncate_ua(value: &str) -> String {
    value.chars().take(512).collect()
}
