//! Usernames and the up-to-three emails of a Knotree account.
//!
//! Sign-in accepts the username or any verified email. Unverified emails never
//! authenticate, so adding one is harmless until its owner confirms it.

use super::session::{self, CurrentSession};
use super::{apply_meta, ClientMeta, NewEvent};
use crate::email::{self, templates};
use crate::error::{is_unique_violation, AppError, AppResult};
use crate::security::password::normalize_email;
use crate::security::random::random_token;
use crate::security::sha256;
use crate::state::AppState;
use chrono::{DateTime, Duration, Utc};
use serde_json::{json, Value};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub const MAX_EMAILS: i64 = 3;
const USERNAME_HOLD_DAYS: i64 = 30;
const USERNAME_CHANGE_DAYS: i64 = 30;

const RESERVED_USERNAMES: &[&str] = &[
    "admin",
    "administrator",
    "root",
    "system",
    "support",
    "help",
    "security",
    "knotree",
    "accounts",
    "account",
    "api",
    "www",
    "mail",
    "oauth",
    "auth",
    "login",
    "logout",
    "signin",
    "signup",
    "sign-in",
    "sign-up",
    "register",
    "settings",
    "me",
    "user",
    "users",
    "null",
    "undefined",
    "registry",
    "cloud",
    "deleted",
];

/// What the user typed into the sign-in box.
#[derive(Debug, PartialEq, Eq)]
pub enum LoginIdentifier {
    Email(String),
    Username(String),
}

impl LoginIdentifier {
    pub fn parse(input: &str) -> Option<Self> {
        let trimmed = input.trim();
        if trimmed.contains('@') {
            normalize_email(trimmed).ok().map(Self::Email)
        } else {
            normalize_username(trimmed).ok().map(Self::Username)
        }
    }

    /// Stable key for rate limiting attempts against one account.
    pub fn rate_subject(input: &str) -> String {
        match Self::parse(input) {
            Some(Self::Email(email)) => format!("email:{email}"),
            Some(Self::Username(username)) => format!("username:{username}"),
            None => format!("raw:{}", input.trim().to_lowercase()),
        }
    }
}

/// Lowercases and validates a username: 3–39 characters of `a-z`, `0-9` and
/// inner hyphens, not one of the reserved names.
pub fn normalize_username(input: &str) -> AppResult<String> {
    let username = input.trim().to_lowercase();
    let length = username.chars().count();
    let shape_ok = (3..=39).contains(&length)
        && username
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !username.starts_with('-')
        && !username.ends_with('-');
    if !shape_ok {
        return Err(AppError::Validation(
            "Usernames are 3 to 39 letters, numbers or hyphens and cannot start or end with a hyphen.",
        ));
    }
    if RESERVED_USERNAMES.contains(&username.as_str()) {
        return Err(AppError::Validation("That username is reserved."));
    }
    Ok(username)
}

async fn username_taken(
    tx: &mut Transaction<'_, Postgres>,
    username: &str,
    claimant: Option<Uuid>,
) -> AppResult<bool> {
    let taken: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (SELECT 1 FROM users WHERE username = $1 AND ($2::uuid IS NULL OR id <> $2))
            OR EXISTS (
                SELECT 1 FROM username_holds
                WHERE username = $1 AND released_until > now() AND ($2::uuid IS NULL OR user_id <> $2)
            )
        "#,
    )
    .bind(username)
    .bind(claimant)
    .fetch_one(&mut **tx)
    .await?;
    Ok(taken)
}

/// Fails with a conflict when the username belongs to, or is held for, someone else.
pub async fn claim_username(
    tx: &mut Transaction<'_, Postgres>,
    username: &str,
    claimant: Option<Uuid>,
) -> AppResult<()> {
    if username_taken(tx, username, claimant).await? {
        return Err(AppError::Conflict("That username is already taken."));
    }
    Ok(())
}

/// Picks a free username for accounts created through a social provider.
pub async fn allocate_username(
    tx: &mut Transaction<'_, Postgres>,
    seed: &str,
) -> AppResult<String> {
    let mut stem: String = seed
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    while stem.contains("--") {
        stem = stem.replace("--", "-");
    }
    let stem: String = stem.trim_matches('-').chars().take(30).collect();
    let stem = stem.trim_matches('-').to_string();
    let stem = if normalize_username(&stem).is_ok() {
        stem
    } else {
        "user".to_string()
    };
    if stem != "user" && !username_taken(tx, &stem, None).await? {
        return Ok(stem);
    }
    for _ in 0..8 {
        let suffix: String = random_token()?
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_lowercase())
            .take(6)
            .collect();
        let candidate = format!("{stem}-{suffix}");
        if normalize_username(&candidate).is_ok() && !username_taken(tx, &candidate, None).await? {
            return Ok(candidate);
        }
    }
    Err(AppError::Conflict("Could not pick a username. Try again."))
}

pub async fn change_username(
    state: &AppState,
    session: &CurrentSession,
    input: &str,
    meta: &ClientMeta,
) -> AppResult<String> {
    session::require_step_up(state, session)?;
    let username = normalize_username(input)?;
    let mut tx = state.db.begin().await?;
    let (current, changed_at): (String, Option<DateTime<Utc>>) =
        sqlx::query_as("SELECT username, username_changed_at FROM users WHERE id = $1 FOR UPDATE")
            .bind(session.user_id)
            .fetch_one(&mut *tx)
            .await?;
    if current == username {
        return Ok(username);
    }
    if changed_at.is_some_and(|at| at + Duration::days(USERNAME_CHANGE_DAYS) > Utc::now()) {
        return Err(AppError::Validation(
            "You can change your username once every 30 days.",
        ));
    }
    claim_username(&mut tx, &username, Some(session.user_id)).await?;
    if let Err(err) = sqlx::query(
        "UPDATE users SET username = $2, username_changed_at = now(), updated_at = now() WHERE id = $1",
    )
    .bind(session.user_id)
    .bind(&username)
    .execute(&mut *tx)
    .await
    {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict("That username is already taken."));
        }
        return Err(err.into());
    }
    sqlx::query("DELETE FROM username_holds WHERE username = $1")
        .bind(&username)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        INSERT INTO username_holds (username, user_id, released_until) VALUES ($1, $2, $3)
        ON CONFLICT (username) DO UPDATE SET user_id = EXCLUDED.user_id, released_until = EXCLUDED.released_until
        "#,
    )
    .bind(&current)
    .bind(session.user_id)
    .bind(Utc::now() + Duration::days(USERNAME_HOLD_DAYS))
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("USERNAME_CHANGED", session.user_id);
    event.metadata = json!({"from": current, "to": username});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    alert_primary(
        state,
        session.user_id,
        templates::SecurityAlert::UsernameChanged,
    )
    .await;
    Ok(username)
}

pub async fn list_emails(state: &AppState, user_id: Uuid) -> AppResult<Vec<Value>> {
    let rows: Vec<(Uuid, String, bool, Option<DateTime<Utc>>, DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT id, email, is_primary, verified_at, created_at
        FROM user_emails
        WHERE user_id = $1
        ORDER BY is_primary DESC, created_at
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, email, primary, verified_at, created_at)| {
            json!({
                "id": id,
                "email": email,
                "primary": primary,
                "verified": verified_at.is_some(),
                "created_at": created_at,
            })
        })
        .collect())
}

/// Frees an address held by someone else's unconfirmed secondary email whose
/// verification window has passed, so an unverified claim cannot block it.
pub(crate) async fn release_stale_claim(
    tx: &mut Transaction<'_, Postgres>,
    email: &str,
    verification_hours: i64,
) -> AppResult<()> {
    sqlx::query(
        r#"
        DELETE FROM user_emails
        WHERE email = $1 AND verified_at IS NULL AND NOT is_primary
          AND created_at < now() - make_interval(hours => $2::int)
        "#,
    )
    .bind(email)
    .bind(verification_hours as i32)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn add_email(
    state: &AppState,
    session: &CurrentSession,
    input: &str,
    meta: &ClientMeta,
) -> AppResult<Uuid> {
    session::require_step_up(state, session)?;
    let email = normalize_email(input)?;
    let mut tx = state.db.begin().await?;
    sqlx::query("SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
        DELETE FROM user_emails
        WHERE user_id = $1 AND verified_at IS NULL AND NOT is_primary
          AND created_at < now() - make_interval(hours => $2::int)
        "#,
    )
    .bind(session.user_id)
    .bind(state.config.verification_hours as i32)
    .execute(&mut *tx)
    .await?;
    release_stale_claim(&mut tx, &email, state.config.verification_hours).await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM user_emails WHERE user_id = $1")
        .bind(session.user_id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= MAX_EMAILS {
        return Err(AppError::Validation(
            "An account can have up to three email addresses.",
        ));
    }
    let id = Uuid::now_v7();
    let now = Utc::now();
    if let Err(err) = sqlx::query(
        "INSERT INTO user_emails (id, user_id, email, is_primary, created_at) VALUES ($1, $2, $3, FALSE, $4)",
    )
    .bind(id)
    .bind(session.user_id)
    .bind(&email)
    .bind(now)
    .execute(&mut *tx)
    .await
    {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict(
                "This email is already used by a Knotree account.",
            ));
        }
        return Err(err.into());
    }
    let token = issue_add_challenge(&mut tx, state, session.user_id, &email, now).await?;
    let mut event = NewEvent::success("EMAIL_ADD_REQUESTED", session.user_id);
    event.metadata = json!({"email_id": id});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    send_add_link(state, session.user_id, &email, &token).await?;
    Ok(id)
}

async fn issue_add_challenge(
    tx: &mut Transaction<'_, Postgres>,
    state: &AppState,
    user_id: Uuid,
    email: &str,
    now: DateTime<Utc>,
) -> AppResult<String> {
    let token = random_token()?;
    sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE user_id = $1 AND purpose = 'email_add' AND email = $2 AND consumed_at IS NULL",
    )
    .bind(user_id)
    .bind(email)
    .execute(&mut **tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO email_challenges (id, user_id, email, purpose, code_hash, created_at, expires_at)
        VALUES ($1, $2, $3, 'email_add', $4, $5, $6)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(email)
    .bind(sha256(token.as_bytes()))
    .bind(now)
    .bind(now + Duration::hours(state.config.verification_hours))
    .execute(&mut **tx)
    .await?;
    Ok(token)
}

async fn send_add_link(state: &AppState, user_id: Uuid, email: &str, token: &str) -> AppResult<()> {
    let link = format!("{}/verify-email?token={token}", state.config.app_base_url);
    email::enqueue_and_send(
        state,
        email,
        templates::added_email_verification(
            crate::i18n::user_locale(&state.db, user_id).await,
            &link,
        ),
    )
    .await?;
    Ok(())
}

pub async fn resend_added_email(
    state: &AppState,
    session: &CurrentSession,
    email_id: Uuid,
) -> AppResult<()> {
    crate::security::rate_limit::enforce(
        state,
        &crate::security::rate_limit::Limit {
            kind: "email_add_resend",
            subject: format!("user:{}", session.user_id),
            limit: 3,
            window_seconds: 900,
        },
        None,
    )
    .await?;
    crate::security::rate_limit::record(
        state,
        "email_add_resend",
        &format!("user:{}", session.user_id),
        None,
        false,
    )
    .await?;
    let mut tx = state.db.begin().await?;
    let email: Option<String> = sqlx::query_scalar(
        "SELECT email FROM user_emails WHERE id = $1 AND user_id = $2 AND verified_at IS NULL AND NOT is_primary",
    )
    .bind(email_id)
    .bind(session.user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(email) = email else {
        return Err(AppError::NotFound);
    };
    sqlx::query("UPDATE user_emails SET created_at = now() WHERE id = $1")
        .bind(email_id)
        .execute(&mut *tx)
        .await?;
    let token = issue_add_challenge(&mut tx, state, session.user_id, &email, Utc::now()).await?;
    tx.commit().await?;
    send_add_link(state, session.user_id, &email, &token).await
}

/// Confirms an added email from its link. Returns `Gone` for unknown tokens so
/// the verify endpoint can try the other email-link kinds.
pub async fn confirm_added_email(
    state: &AppState,
    token: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, Uuid, String)> = sqlx::query_as(
        r#"
        SELECT id, user_id, email FROM email_challenges
        WHERE purpose = 'email_add' AND code_hash = $1 AND consumed_at IS NULL AND expires_at > now()
        FOR UPDATE
        "#,
    )
    .bind(sha256(token.as_bytes()))
    .fetch_optional(&mut *tx)
    .await?;
    let Some((challenge_id, user_id, email)) = row else {
        return Err(AppError::Gone(
            "This verification link is invalid or expired.",
        ));
    };
    sqlx::query("UPDATE email_challenges SET consumed_at = now() WHERE id = $1")
        .bind(challenge_id)
        .execute(&mut *tx)
        .await?;
    let updated = sqlx::query(
        "UPDATE user_emails SET verified_at = now() WHERE user_id = $1 AND email = $2 AND verified_at IS NULL",
    )
    .bind(user_id)
    .bind(&email)
    .execute(&mut *tx)
    .await?;
    if updated.rows_affected() != 1 {
        tx.commit().await?;
        return Err(AppError::Gone(
            "This verification link is invalid or expired.",
        ));
    }
    let mut event = NewEvent::success("EMAIL_ADDED", user_id);
    event.metadata = json!({"email": email});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    alert_primary(state, user_id, templates::SecurityAlert::EmailAdded).await;
    Ok(())
}

pub async fn make_primary(
    state: &AppState,
    session: &CurrentSession,
    email_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<()> {
    session::require_step_up(state, session)?;
    let mut tx = state.db.begin().await?;
    let target: Option<(String, Option<DateTime<Utc>>, bool)> = sqlx::query_as(
        "SELECT email, verified_at, is_primary FROM user_emails WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(email_id)
    .bind(session.user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((email, verified_at, is_primary)) = target else {
        return Err(AppError::NotFound);
    };
    if is_primary {
        return Ok(());
    }
    if verified_at.is_none() {
        return Err(AppError::Validation(
            "Verify this email before making it primary.",
        ));
    }
    let previous: Option<String> =
        sqlx::query_scalar("SELECT email FROM user_emails WHERE user_id = $1 AND is_primary")
            .bind(session.user_id)
            .fetch_optional(&mut *tx)
            .await?;
    sqlx::query("UPDATE user_emails SET is_primary = FALSE WHERE user_id = $1 AND is_primary")
        .bind(session.user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("UPDATE user_emails SET is_primary = TRUE WHERE id = $1")
        .bind(email_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE identities SET email = $2, email_verified = TRUE WHERE user_id = $1 AND provider = 'password'",
    )
    .bind(session.user_id)
    .bind(&email)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("PRIMARY_EMAIL_CHANGED", session.user_id);
    event.metadata = json!({"email_id": email_id});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    let locale = crate::i18n::user_locale(&state.db, session.user_id).await;
    let security_url = format!("{}/account/security", state.config.app_base_url);
    for address in previous.iter().chain(std::iter::once(&email)) {
        let _ = email::enqueue_and_send(
            state,
            address,
            templates::security_alert(
                locale,
                templates::SecurityAlert::EmailChanged,
                &security_url,
            ),
        )
        .await;
    }
    Ok(())
}

pub async fn remove_email(
    state: &AppState,
    session: &CurrentSession,
    email_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<()> {
    session::require_step_up(state, session)?;
    let mut tx = state.db.begin().await?;
    let target: Option<(String, bool, Option<DateTime<Utc>>)> = sqlx::query_as(
        "SELECT email, is_primary, verified_at FROM user_emails WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(email_id)
    .bind(session.user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((email, is_primary, verified_at)) = target else {
        return Err(AppError::NotFound);
    };
    if is_primary {
        return Err(AppError::Validation(
            "Make another email primary before removing this one.",
        ));
    }
    sqlx::query("DELETE FROM user_emails WHERE id = $1")
        .bind(email_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE user_id = $1 AND email = $2 AND consumed_at IS NULL",
    )
    .bind(session.user_id)
    .bind(&email)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("EMAIL_REMOVED", session.user_id);
    event.metadata = json!({"email_id": email_id});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    if verified_at.is_some() {
        alert_primary(
            state,
            session.user_id,
            templates::SecurityAlert::EmailRemoved,
        )
        .await;
    }
    Ok(())
}

async fn alert_primary(state: &AppState, user_id: Uuid, alert: templates::SecurityAlert) {
    let Ok(Some((primary, _))) = super::primary_email(&state.db, user_id).await else {
        return;
    };
    let _ = email::enqueue_and_send(
        state,
        &primary,
        templates::security_alert(
            crate::i18n::user_locale(&state.db, user_id).await,
            alert,
            &format!("{}/account/security", state.config.app_base_url),
        ),
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_lowercase_bounded_and_not_reserved() {
        assert_eq!(
            normalize_username(" Ada-Lovelace ").unwrap(),
            "ada-lovelace"
        );
        assert!(normalize_username("ab").is_err());
        assert!(normalize_username("-ada").is_err());
        assert!(normalize_username("ada-").is_err());
        assert!(normalize_username("ada_l").is_err());
        assert!(normalize_username("admin").is_err());
        assert!(normalize_username(&"a".repeat(40)).is_err());
        assert!(normalize_username(&"a".repeat(39)).is_ok());
    }

    #[test]
    fn identifiers_split_on_the_at_sign() {
        assert_eq!(
            LoginIdentifier::parse(" Ada@Example.com "),
            Some(LoginIdentifier::Email("ada@example.com".into()))
        );
        assert_eq!(
            LoginIdentifier::parse("Ada"),
            Some(LoginIdentifier::Username("ada".into()))
        );
        assert_eq!(LoginIdentifier::parse("not valid!"), None);
    }
}
