mod account;
mod authorizations;
mod bootstrap;
mod clients;
mod events;
mod identity;
mod login;
mod mfa;
mod password;
mod register;
mod session;
mod social;

pub use account::*;
pub use authorizations::*;
pub use bootstrap::bootstrap_admin;
pub use clients::ensure_dev_redirects;
pub use events::{record, truncate_ua, NewEvent};
pub use identity::*;
pub use login::*;
pub use mfa::*;
pub use password::*;
pub use register::*;
pub use session::*;
pub use social::*;

use crate::error::AppResult;
use chrono::{DateTime, Utc};
use ipnetwork::IpNetwork;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ClientMeta {
    pub request_id: String,
    pub ip: Option<IpNetwork>,
    pub user_agent: Option<String>,
    pub locale: crate::i18n::Locale,
}

pub fn apply_meta(event: &mut NewEvent, meta: &ClientMeta) {
    event.request_id = Some(meta.request_id.clone());
    event.ip = meta.ip;
    event.user_agent = meta.user_agent.clone();
}

pub async fn primary_email(
    db: &sqlx::PgPool,
    user_id: Uuid,
) -> AppResult<Option<(String, Option<DateTime<Utc>>)>> {
    let row: Option<(String, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT email, verified_at FROM user_emails WHERE user_id = $1 AND is_primary = TRUE",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    Ok(row)
}
