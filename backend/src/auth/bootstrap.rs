use crate::error::AppResult;
use crate::security::password::normalize_email;
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
