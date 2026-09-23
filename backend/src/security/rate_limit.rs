use crate::error::{AppError, AppResult};
use crate::state::AppState;
use ipnetwork::IpNetwork;

pub struct Limit {
    pub kind: &'static str,
    pub subject: String,
    pub limit: i64,
    pub window_seconds: i64,
}

pub async fn enforce(state: &AppState, limit: &Limit, ip: Option<IpNetwork>) -> AppResult<()> {
    let count = count_subject(state, limit.kind, &limit.subject, limit.window_seconds).await?;
    if count >= limit.limit {
        return Err(AppError::RateLimited {
            retry_after_seconds: limit.window_seconds as u64,
        });
    }
    if let Some(ip) = ip {
        let ip_subject = format!("ip:{ip}");
        let ip_count = count_subject(state, limit.kind, &ip_subject, limit.window_seconds).await?;
        if ip_count >= limit.limit.saturating_mul(4).max(limit.limit) {
            return Err(AppError::RateLimited {
                retry_after_seconds: limit.window_seconds as u64,
            });
        }
    }
    Ok(())
}

async fn count_subject(
    state: &AppState,
    kind: &str,
    subject: &str,
    window_seconds: i64,
) -> AppResult<i64> {
    let count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)
        FROM auth_attempts
        WHERE kind = $1
          AND subject = $2
          AND success = FALSE
          AND occurred_at > now() - ($3::int * interval '1 second')
        "#,
    )
    .bind(kind)
    .bind(subject)
    .bind(window_seconds as i32)
    .fetch_one(&state.db)
    .await?;
    Ok(count)
}

pub async fn record(
    state: &AppState,
    kind: &str,
    subject: &str,
    ip: Option<IpNetwork>,
    success: bool,
) -> AppResult<()> {
    sqlx::query(
        r#"
        INSERT INTO auth_attempts (kind, subject, ip, success)
        VALUES ($1, $2, $3, $4)
        "#,
    )
    .bind(kind)
    .bind(subject)
    .bind(ip)
    .bind(success)
    .execute(&state.db)
    .await?;
    if let Some(ip) = ip {
        sqlx::query(
            r#"
            INSERT INTO auth_attempts (kind, subject, ip, success)
            VALUES ($1, $2, $3, $4)
            "#,
        )
        .bind(kind)
        .bind(format!("ip:{ip}"))
        .bind(ip)
        .bind(success)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}
