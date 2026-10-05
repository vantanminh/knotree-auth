use crate::error::AppResult;
use crate::security::password::{hash_password, normalize_email, validate_password};
use crate::state::AppState;
use crate::AppError;
use chrono::Utc;
use uuid::Uuid;

pub async fn bootstrap_admin(state: &AppState) -> AppResult<()> {
    if let Some(user_id) = state.config.super_admin_user_id {
        let eligible: bool = sqlx::query_scalar(
            r#"
            SELECT EXISTS(
                SELECT 1
                FROM users u
                JOIN user_emails e ON e.user_id = u.id AND e.is_primary
                WHERE u.id = $1 AND u.status = 'active' AND e.verified_at IS NOT NULL
            )
            "#,
        )
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
        if !eligible {
            return Err(AppError::internal(
                "SUPER_ADMIN_USER_ID must reference an active user with a verified primary email",
            ));
        }
        let mut tx = state.db.begin().await?;
        sqlx::query("DELETE FROM role_assignments WHERE role = 'super_admin' AND user_id <> $1")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
        sqlx::query(
            r#"
            INSERT INTO role_assignments (user_id, role, created_at)
            VALUES ($1, 'super_admin', $2)
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(user_id)
        .bind(Utc::now())
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        tracing::info!(%user_id, "super admin role ensured from SUPER_ADMIN_USER_ID");
        return Ok(());
    }

    let Some(email) = state.config.super_admin_email.clone() else {
        return Ok(());
    };
    if let Some(password) = state.config.super_admin_password.clone() {
        return seed_super_admin(state, &email, &password).await;
    }
    if state.config.env.is_production() {
        tracing::warn!("SUPER_ADMIN_EMAIL is ignored in production; set SUPER_ADMIN_USER_ID");
        return Ok(());
    }
    let email = normalize_email(&email).unwrap_or(email);
    let user_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        SELECT u.id
        FROM users u
        JOIN user_emails e ON e.user_id = u.id AND e.is_primary
        WHERE e.email = $1 AND e.verified_at IS NOT NULL AND u.status = 'active'
        "#,
    )
    .bind(&email)
    .fetch_optional(&state.db)
    .await?;
    let Some(user_id) = user_id else {
        tracing::info!("super admin bootstrap is waiting for a verified account");
        return Ok(());
    };
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM role_assignments WHERE role = 'super_admin' LIMIT 1",
    )
    .fetch_optional(&state.db)
    .await?;
    if let Some(existing) = existing {
        if existing != user_id {
            tracing::warn!("super admin already assigned; not replacing from email");
        }
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO role_assignments (user_id, role, created_at) VALUES ($1, 'super_admin', $2)",
    )
    .bind(user_id)
    .bind(Utc::now())
    .execute(&state.db)
    .await?;
    tracing::info!(%user_id, "development super admin bootstrapped; set SUPER_ADMIN_USER_ID to this id");
    Ok(())
}

/// Creates the super admin account from `SUPER_ADMIN_EMAIL` and
/// `SUPER_ADMIN_PASSWORD` when it does not exist, then makes it the only super
/// admin. An existing account keeps its current password so a password changed
/// in the app survives restarts.
async fn seed_super_admin(state: &AppState, email: &str, password: &str) -> AppResult<()> {
    let email = normalize_email(email)
        .map_err(|_| AppError::internal("SUPER_ADMIN_EMAIL is not a valid email"))?;
    let mut tx = state.db.begin().await?;
    let existing: Option<Uuid> =
        sqlx::query_scalar("SELECT user_id FROM user_emails WHERE email = $1")
            .bind(&email)
            .fetch_optional(&mut *tx)
            .await?;
    let now = Utc::now();
    let user_id = match existing {
        Some(user_id) => {
            sqlx::query(
                "UPDATE user_emails SET verified_at = COALESCE(verified_at, $2) WHERE user_id = $1 AND email = $3",
            )
            .bind(user_id)
            .bind(now)
            .bind(&email)
            .execute(&mut *tx)
            .await?;
            user_id
        }
        None => {
            validate_password(password, &email).map_err(|err| {
                AppError::internal(format!(
                    "SUPER_ADMIN_PASSWORD does not meet the password policy: {err}"
                ))
            })?;
            let username = super::identity::normalize_username(&state.config.super_admin_username)
                .map_err(|_| AppError::internal("SUPER_ADMIN_USERNAME is not a valid username"))?;
            super::identity::claim_username(&mut tx, &username, None)
                .await
                .map_err(|_| AppError::internal("SUPER_ADMIN_USERNAME is already taken"))?;
            let password_hash = hash_password(password, &state.config.argon)?;
            let user_id = Uuid::now_v7();
            let identity_id = Uuid::now_v7();
            sqlx::query(
                r#"
                INSERT INTO users (id, username, display_name, status, created_at, updated_at, password_changed_at)
                VALUES ($1, $2, 'Super Admin', 'active', $3, $3, $3)
                "#,
            )
            .bind(user_id)
            .bind(&username)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO user_emails (id, user_id, email, is_primary, verified_at, created_at)
                VALUES ($1, $2, $3, TRUE, $4, $4)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(user_id)
            .bind(&email)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
                INSERT INTO identities (id, user_id, provider, provider_subject, email, email_verified, created_at)
                VALUES ($1, $2, 'password', $3, $4, TRUE, $5)
                "#,
            )
            .bind(identity_id)
            .bind(user_id)
            .bind(user_id.to_string())
            .bind(&email)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                "INSERT INTO password_credentials (identity_id, password_hash, updated_at) VALUES ($1, $2, $3)",
            )
            .bind(identity_id)
            .bind(&password_hash)
            .bind(now)
            .execute(&mut *tx)
            .await?;
            tracing::info!(%user_id, "super admin account created from SUPER_ADMIN_EMAIL");
            user_id
        }
    };
    sqlx::query("DELETE FROM role_assignments WHERE role = 'super_admin' AND user_id <> $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO role_assignments (user_id, role, created_at) VALUES ($1, 'super_admin', $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::info!(%user_id, "super admin role ensured from SUPER_ADMIN_EMAIL");
    Ok(())
}
