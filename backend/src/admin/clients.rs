//! Super admin management of services (OAuth clients): create, edit, rotate
//! the secret and manage the square logo shown on the consent screen.

use crate::auth::{self, ClientMeta};
use crate::error::{is_unique_violation, AppError, AppResult};
use crate::security::random::random_token;
use crate::security::sha256;
use crate::state::AppState;
use chrono::Utc;
use image::imageops::FilterType;
use image::{DynamicImage, GenericImageView, ImageFormat};
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::Cursor;
use std::path::PathBuf;
use uuid::Uuid;

pub const LOGO_MAX_BYTES: usize = 1024 * 1024;
const LOGO_SIZE: u32 = 512;
const LOGO_MIN_SIZE: u32 = 64;

#[derive(Deserialize)]
pub struct ClientInput {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub homepage_url: Option<String>,
    pub client_type: Option<String>,
    pub redirect_uris: Option<Vec<String>>,
    pub allowed_scopes: Option<Vec<String>>,
    pub first_party: Option<bool>,
    pub require_pkce: Option<bool>,
    pub status: Option<String>,
}

/// The public URL of a stored logo file, served by `logo_file`.
pub fn logo_url(path: Option<&str>) -> Option<String> {
    path.map(|file| format!("/api/v1/client-logos/{file}"))
}

fn validate_id(id: &str) -> AppResult<String> {
    let id = id.trim().to_lowercase();
    let ok = (3..=64).contains(&id.len())
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !id.starts_with('-')
        && !id.ends_with('-');
    if !ok {
        return Err(AppError::Validation(
            "Service IDs are 3 to 64 lowercase letters, numbers or hyphens.",
        ));
    }
    Ok(id)
}

fn validate_name(name: &str) -> AppResult<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(AppError::Validation(
            "Enter a service name up to 80 characters.",
        ));
    }
    Ok(name.to_string())
}

fn validate_description(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    else {
        return Ok(None);
    };
    if value.chars().count() > 1000 {
        return Err(AppError::Validation(
            "Keep the description under 1000 characters.",
        ));
    }
    Ok(Some(value))
}

fn validate_homepage(value: Option<String>) -> AppResult<Option<String>> {
    let Some(value) = value
        .map(|v| v.trim().to_string())
        .filter(|v| !v.is_empty())
    else {
        return Ok(None);
    };
    match url::Url::parse(&value) {
        Ok(url) if matches!(url.scheme(), "https" | "http") && url.host().is_some() => {
            Ok(Some(value))
        }
        _ => Err(AppError::Validation("Enter a valid homepage URL.")),
    }
}

fn validate_redirects(values: Vec<String>, production: bool) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for value in values.into_iter().map(|v| v.trim().to_string()) {
        if value.is_empty() {
            continue;
        }
        let Ok(url) = url::Url::parse(&value) else {
            return Err(AppError::Validation(
                "Each redirect URI must be an absolute URL.",
            ));
        };
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        let scheme_ok =
            url.scheme() == "https" || (url.scheme() == "http" && (local || !production));
        if !scheme_ok || url.fragment().is_some() || url.host().is_none() {
            return Err(AppError::Validation(
                "Redirect URIs must use HTTPS (HTTP only for localhost) and have no fragment.",
            ));
        }
        if !out.contains(&value) {
            out.push(value);
        }
    }
    if out.len() > 20 {
        return Err(AppError::Validation("Use at most 20 redirect URIs."));
    }
    Ok(out)
}

fn validate_scopes(values: Vec<String>) -> AppResult<Vec<String>> {
    let mut out = Vec::new();
    for scope in values.into_iter().map(|v| v.trim().to_string()) {
        if scope.is_empty() {
            continue;
        }
        let ok = scope.len() <= 64
            && scope
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | ':' | '.' | '-'));
        if !ok {
            return Err(AppError::Validation(
                "Scopes use letters, numbers and _ : . - only.",
            ));
        }
        if !out.contains(&scope) {
            out.push(scope);
        }
    }
    if !out.iter().any(|s| s == "openid") {
        out.insert(0, "openid".into());
    }
    Ok(out)
}

fn validate_type(value: &str) -> AppResult<&'static str> {
    match value {
        "public" => Ok("public"),
        "confidential" => Ok("confidential"),
        "service" => Ok("service"),
        _ => Err(AppError::Validation("Choose a valid service type.")),
    }
}

fn validate_status(value: &str) -> AppResult<&'static str> {
    match value {
        "active" => Ok("active"),
        "disabled" => Ok("disabled"),
        _ => Err(AppError::Validation("Status must be active or disabled.")),
    }
}

async fn audit(
    state: &AppState,
    actor: Uuid,
    client_id: &str,
    action: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let mut event = auth::NewEvent::success("ADMIN_ACTION", actor);
    event.client_id = Some(client_id.to_string());
    event.metadata = json!({"action": action, "client_id": client_id});
    auth::apply_meta(&mut event, meta);
    auth::record(&state.db, event).await?;
    Ok(())
}

pub async fn create_client(
    state: &AppState,
    actor: Uuid,
    input: ClientInput,
    meta: &ClientMeta,
) -> AppResult<Value> {
    let id = validate_id(input.id.as_deref().unwrap_or_default())?;
    let name = validate_name(input.name.as_deref().unwrap_or_default())?;
    let description = validate_description(input.description)?;
    let homepage_url = validate_homepage(input.homepage_url)?;
    let client_type = validate_type(input.client_type.as_deref().unwrap_or("public"))?;
    let redirect_uris = validate_redirects(
        input.redirect_uris.unwrap_or_default(),
        state.config.env.is_production(),
    )?;
    if client_type != "service" && redirect_uris.is_empty() {
        return Err(AppError::Validation("Add at least one redirect URI."));
    }
    let allowed_scopes = validate_scopes(
        input
            .allowed_scopes
            .unwrap_or_else(|| vec!["openid".into(), "profile".into(), "email".into()]),
    )?;
    let secret = if client_type == "public" {
        None
    } else {
        Some(random_token()?)
    };
    let require_pkce = input.require_pkce.unwrap_or(true) || client_type == "public";
    let inserted = sqlx::query(
        r#"
        INSERT INTO oauth_clients (
            id, name, description, homepage_url, client_type, secret_hash, redirect_uris,
            allowed_scopes, first_party, require_pkce, status, created_at, updated_at
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, 'active', $11, $11)
        "#,
    )
    .bind(&id)
    .bind(&name)
    .bind(&description)
    .bind(&homepage_url)
    .bind(client_type)
    .bind(secret.as_ref().map(|s| sha256(s.as_bytes())))
    .bind(&redirect_uris)
    .bind(&allowed_scopes)
    .bind(input.first_party.unwrap_or(false))
    .bind(require_pkce)
    .bind(Utc::now())
    .execute(&state.db)
    .await;
    if let Err(err) = inserted {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict("A service with this ID already exists."));
        }
        return Err(err.into());
    }
    audit(state, actor, &id, "client_create", meta).await?;
    let mut value = super::client_detail(state, &id).await?;
    value["client_secret"] = json!(secret);
    Ok(value)
}

pub async fn update_client(
    state: &AppState,
    actor: Uuid,
    id: &str,
    input: ClientInput,
    meta: &ClientMeta,
) -> AppResult<Value> {
    let current: Option<(String, String)> =
        sqlx::query_as("SELECT client_type, name FROM oauth_clients WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some((client_type, _)) = current else {
        return Err(AppError::NotFound);
    };
    let name = input.name.as_deref().map(validate_name).transpose()?;
    let description = input
        .description
        .map(|d| validate_description(Some(d)))
        .transpose()?;
    let homepage_url = input
        .homepage_url
        .map(|h| validate_homepage(Some(h)))
        .transpose()?;
    let redirect_uris = input
        .redirect_uris
        .map(|r| validate_redirects(r, state.config.env.is_production()))
        .transpose()?;
    if client_type != "service" && redirect_uris.as_ref().is_some_and(Vec::is_empty) {
        return Err(AppError::Validation("Add at least one redirect URI."));
    }
    let allowed_scopes = input.allowed_scopes.map(validate_scopes).transpose()?;
    let status = input.status.as_deref().map(validate_status).transpose()?;
    let require_pkce = input.require_pkce.map(|p| p || client_type == "public");
    sqlx::query(
        r#"
        UPDATE oauth_clients SET
            name = COALESCE($2, name),
            description = CASE WHEN $3 THEN $4 ELSE description END,
            homepage_url = CASE WHEN $5 THEN $6 ELSE homepage_url END,
            redirect_uris = COALESCE($7, redirect_uris),
            allowed_scopes = COALESCE($8, allowed_scopes),
            first_party = COALESCE($9, first_party),
            require_pkce = COALESCE($10, require_pkce),
            status = COALESCE($11, status),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(id)
    .bind(name)
    .bind(description.is_some())
    .bind(description.flatten())
    .bind(homepage_url.is_some())
    .bind(homepage_url.flatten())
    .bind(redirect_uris)
    .bind(allowed_scopes)
    .bind(input.first_party)
    .bind(require_pkce)
    .bind(status)
    .execute(&state.db)
    .await?;
    if status == Some("disabled") {
        sqlx::query(
            "UPDATE refresh_tokens SET revoked_at = now() WHERE client_id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&state.db)
        .await?;
        sqlx::query(
            "UPDATE oauth_access_tokens SET revoked_at = now() WHERE client_id = $1 AND revoked_at IS NULL",
        )
        .bind(id)
        .execute(&state.db)
        .await?;
    }
    audit(state, actor, id, "client_update", meta).await?;
    super::client_detail(state, id).await
}

pub async fn rotate_secret(
    state: &AppState,
    actor: Uuid,
    id: &str,
    meta: &ClientMeta,
) -> AppResult<Value> {
    let client_type: Option<String> =
        sqlx::query_scalar("SELECT client_type FROM oauth_clients WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    match client_type.as_deref() {
        None => return Err(AppError::NotFound),
        Some("public") => return Err(AppError::Validation("Public services do not use a secret.")),
        _ => {}
    }
    let secret = random_token()?;
    sqlx::query("UPDATE oauth_clients SET secret_hash = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(sha256(secret.as_bytes()))
        .execute(&state.db)
        .await?;
    audit(state, actor, id, "client_rotate_secret", meta).await?;
    Ok(json!({"client_secret": secret}))
}

/// Decodes an uploaded logo, checks it is (nearly) square, centre-crops it to
/// an exact square and re-encodes it as a PNG of at most 512×512. Re-encoding
/// drops any metadata or payload smuggled in the original file.
pub fn normalize_logo(bytes: &[u8]) -> AppResult<Vec<u8>> {
    if bytes.len() > LOGO_MAX_BYTES {
        return Err(AppError::Validation("The logo must be 1 MB or smaller."));
    }
    let format = image::guess_format(bytes)
        .map_err(|_| AppError::Validation("Upload a PNG, JPEG or WebP image."))?;
    if !matches!(
        format,
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP
    ) {
        return Err(AppError::Validation("Upload a PNG, JPEG or WebP image."));
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), format);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(4096);
    limits.max_image_height = Some(4096);
    limits.max_alloc = Some(64 * 1024 * 1024);
    reader.limits(limits);
    let decoded = reader
        .decode()
        .map_err(|_| AppError::Validation("Upload a PNG, JPEG or WebP image."))?;
    let (width, height) = decoded.dimensions();
    let side = width.min(height);
    if side < LOGO_MIN_SIZE {
        return Err(AppError::Validation(
            "The logo must be at least 64×64 pixels.",
        ));
    }
    // Allow up to 2% difference so exports that are off by a pixel still work.
    if width.max(height) - side > side / 50 {
        return Err(AppError::Validation("The logo must be a square image."));
    }
    let square = decoded.crop_imm((width - side) / 2, (height - side) / 2, side, side);
    let resized = if side > LOGO_SIZE {
        square.resize_exact(LOGO_SIZE, LOGO_SIZE, FilterType::Lanczos3)
    } else {
        square
    };
    let mut out = Vec::new();
    DynamicImage::ImageRgba8(resized.to_rgba8())
        .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
        .map_err(AppError::internal)?;
    Ok(out)
}

fn logo_dir(state: &AppState) -> &PathBuf {
    &state.config.client_logo_dir
}

pub async fn set_logo(
    state: &AppState,
    actor: Uuid,
    id: &str,
    bytes: &[u8],
    meta: &ClientMeta,
) -> AppResult<Value> {
    let previous: Option<Option<String>> =
        sqlx::query_scalar("SELECT logo_path FROM oauth_clients WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some(previous) = previous else {
        return Err(AppError::NotFound);
    };
    let png = normalize_logo(bytes)?;
    let digest = data_encoding::HEXLOWER.encode(&sha256(&png));
    let file = format!("{id}-{}.png", &digest[..16]);
    tokio::fs::create_dir_all(logo_dir(state))
        .await
        .map_err(AppError::internal)?;
    let tmp = logo_dir(state).join(format!(".{file}.{}", Uuid::now_v7()));
    tokio::fs::write(&tmp, &png)
        .await
        .map_err(AppError::internal)?;
    tokio::fs::rename(&tmp, logo_dir(state).join(&file))
        .await
        .map_err(AppError::internal)?;
    sqlx::query("UPDATE oauth_clients SET logo_path = $2, updated_at = now() WHERE id = $1")
        .bind(id)
        .bind(&file)
        .execute(&state.db)
        .await?;
    if let Some(old) = previous.filter(|old| *old != file) {
        let _ = tokio::fs::remove_file(logo_dir(state).join(old)).await;
    }
    audit(state, actor, id, "client_logo_set", meta).await?;
    Ok(json!({"logo_url": logo_url(Some(&file))}))
}

pub async fn remove_logo(
    state: &AppState,
    actor: Uuid,
    id: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    let previous: Option<Option<String>> =
        sqlx::query_scalar("SELECT logo_path FROM oauth_clients WHERE id = $1")
            .bind(id)
            .fetch_optional(&state.db)
            .await?;
    let Some(previous) = previous else {
        return Err(AppError::NotFound);
    };
    sqlx::query("UPDATE oauth_clients SET logo_path = NULL, updated_at = now() WHERE id = $1")
        .bind(id)
        .execute(&state.db)
        .await?;
    if let Some(old) = previous {
        let _ = tokio::fs::remove_file(logo_dir(state).join(old)).await;
    }
    audit(state, actor, id, "client_logo_remove", meta).await?;
    Ok(())
}

/// Reads a stored logo. Only names produced by `set_logo` are accepted, so the
/// path can never escape the logo directory.
pub async fn logo_file(state: &AppState, file: &str) -> AppResult<Vec<u8>> {
    let valid = file.len() <= 100
        && file.ends_with(".png")
        && file
            .trim_end_matches(".png")
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !file.starts_with('-');
    if !valid {
        return Err(AppError::NotFound);
    }
    tokio::fs::read(logo_dir(state).join(file))
        .await
        .map_err(|_| AppError::NotFound)
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::RgbaImage;

    fn png(width: u32, height: u32) -> Vec<u8> {
        let mut out = Vec::new();
        DynamicImage::ImageRgba8(RgbaImage::new(width, height))
            .write_to(&mut Cursor::new(&mut out), ImageFormat::Png)
            .unwrap();
        out
    }

    #[test]
    fn logos_must_be_square_and_are_capped_at_512() {
        assert!(normalize_logo(&png(300, 200)).is_err());
        assert!(normalize_logo(&png(32, 32)).is_err());
        assert!(normalize_logo(b"not an image").is_err());
        let out = normalize_logo(&png(1000, 1010)).unwrap();
        let decoded = image::load_from_memory(&out).unwrap();
        assert_eq!(decoded.dimensions(), (512, 512));
        let small = image::load_from_memory(&normalize_logo(&png(128, 128)).unwrap()).unwrap();
        assert_eq!(small.dimensions(), (128, 128));
    }

    #[test]
    fn ids_and_redirects_are_validated() {
        assert!(validate_id("my-service").is_ok());
        assert!(validate_id("-bad").is_err());
        assert!(validate_id("../x").is_err());
        assert!(validate_redirects(vec!["http://example.com/cb".into()], true).is_err());
        assert!(validate_redirects(vec!["http://localhost:3000/cb".into()], true).is_ok());
        assert!(validate_redirects(vec!["https://app.example.com/cb#x".into()], true).is_err());
        assert_eq!(
            validate_scopes(vec!["email".into()]).unwrap(),
            vec!["openid".to_string(), "email".to_string()]
        );
    }
}
