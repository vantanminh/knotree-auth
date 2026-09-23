use crate::error::AppResult;
use crate::state::AppState;

pub async fn ensure_dev_redirects(state: &AppState) -> AppResult<()> {
    if state.config.env.is_production() {
        return Ok(());
    }
    let extras = [
        (
            "knotree-study",
            vec![
                "http://localhost:4174/auth/callback".to_string(),
                "http://127.0.0.1:4174/auth/callback".to_string(),
            ],
        ),
        (
            "knotree-app",
            vec!["http://localhost:4175/auth/callback".to_string()],
        ),
    ];
    for (client_id, uris) in extras {
        sqlx::query(
            r#"
            UPDATE oauth_clients
            SET redirect_uris = (
                SELECT ARRAY(SELECT DISTINCT uri FROM unnest(redirect_uris || $2::text[]) AS uri)
            )
            WHERE id = $1
            "#,
        )
        .bind(client_id)
        .bind(&uris)
        .execute(&state.db)
        .await?;
    }
    Ok(())
}
