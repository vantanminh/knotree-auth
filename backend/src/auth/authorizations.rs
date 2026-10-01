use super::{apply_meta, ClientMeta, NewEvent};
use crate::error::{AppError, AppResult};
use crate::state::AppState;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Services (OAuth clients) the user has authorized, newest grant first.
pub async fn list_authorizations(
    state: &AppState,
    user_id: Uuid,
) -> AppResult<Vec<serde_json::Value>> {
    let rows: Vec<(String, String, bool, String, Vec<String>, DateTime<Utc>, Option<DateTime<Utc>>, i64)> = sqlx::query_as(
        r#"
        SELECT c.id, c.name, c.first_party, c.status, oc.scopes, oc.granted_at,
               GREATEST(
                   (SELECT max(created_at) FROM refresh_tokens r WHERE r.user_id = oc.user_id AND r.client_id = oc.client_id),
                   (SELECT max(created_at) FROM oauth_access_tokens a WHERE a.user_id = oc.user_id AND a.client_id = oc.client_id)
               ) AS last_used_at,
               (SELECT count(*) FROM refresh_tokens r
                WHERE r.user_id = oc.user_id AND r.client_id = oc.client_id
                  AND r.revoked_at IS NULL AND r.used_at IS NULL AND r.expires_at > now()) AS active_grants
        FROM oauth_consents oc
        JOIN oauth_clients c ON c.id = oc.client_id
        WHERE oc.user_id = $1
        ORDER BY oc.granted_at DESC
        "#,
    )
    .bind(user_id)
    .fetch_all(&state.db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| {
            serde_json::json!({
                "client_id": row.0,
                "name": row.1,
                "first_party": row.2,
                "status": row.3,
                "scopes": row.4,
                "granted_at": row.5,
                "last_used_at": row.6,
                "active_grants": row.7,
            })
        })
        .collect())
}

/// Removes the user's consent for a client and revokes every token the
/// client holds for that user, signing them out of the service.
pub async fn revoke_authorization(
    state: &AppState,
    user_id: Uuid,
    client_id: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let mut tx = state.db.begin().await?;
    let deleted = sqlx::query("DELETE FROM oauth_consents WHERE user_id = $1 AND client_id = $2")
        .bind(user_id)
        .bind(client_id)
        .execute(&mut *tx)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let refresh = sqlx::query(
        "UPDATE refresh_tokens SET revoked_at = now() WHERE user_id = $1 AND client_id = $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(client_id)
    .execute(&mut *tx)
    .await?;
    let access = sqlx::query(
        "UPDATE oauth_access_tokens SET revoked_at = now() WHERE user_id = $1 AND client_id = $2 AND revoked_at IS NULL",
    )
    .bind(user_id)
    .bind(client_id)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("OAUTH_CONSENT_REVOKED", user_id);
    event.client_id = Some(client_id.to_string());
    event.metadata = serde_json::json!({
        "refresh_tokens_revoked": refresh.rows_affected(),
        "access_tokens_revoked": access.rows_affected(),
    });
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    Ok(())
}
