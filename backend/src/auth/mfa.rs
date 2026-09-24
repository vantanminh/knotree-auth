use super::login::{enabled_mfa_methods, load_login_user};
use super::session::{self, IssueParams, IssuedSession};
use super::{apply_meta, primary_email, ClientMeta, NewEvent};
use crate::email::{self, templates};
use crate::error::{AppError, AppResult};
use crate::security::crypto::TotpKeyring;
use crate::security::random::{normalize_recovery_code, recovery_code};
use crate::security::rate_limit::{self, Limit};
use crate::security::{ct_eq_str, sha256};
use crate::state::AppState;
use chrono::Utc;
use data_encoding::BASE32_NOPAD;
use qrcode::render::svg;
use qrcode::QrCode;
use serde_json::json;
use totp_rs::{Algorithm, TOTP};
use uuid::Uuid;

pub struct TotpSetup {
    pub secret: String,
    pub otpauth_uri: String,
    pub qr_svg: String,
}

pub async fn begin_totp(
    state: &AppState,
    user_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<TotpSetup> {
    session_user_must_be_active(state, user_id).await?;
    if enabled_mfa_methods(state, user_id)
        .await?
        .iter()
        .any(|m| m == "totp")
    {
        return Err(AppError::Conflict("Authenticator is already enabled."));
    }
    let secret = crate::security::random::random_bytes(20)?;
    let (email, _) = primary_email(&state.db, user_id)
        .await?
        .ok_or(AppError::Validation(
            "Add an email before enabling an authenticator.",
        ))?;
    let totp = build_totp(&secret, &email)?;
    let (version, nonce, ciphertext) = state.config.totp_keys.encrypt(&secret)?;
    let now = Utc::now();
    let mut tx = state.db.begin().await?;
    let method_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO mfa_methods (id, user_id, method, created_at)
        VALUES ($1, $2, 'totp', $3)
        ON CONFLICT (user_id, method) DO UPDATE SET disabled_at = NULL, enabled_at = NULL
        RETURNING id
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        INSERT INTO totp_credentials (
            mfa_method_id, secret_ciphertext, secret_nonce, key_version, confirmed_at, last_used_step
        ) VALUES ($1,$2,$3,$4,NULL,NULL)
        ON CONFLICT (mfa_method_id) DO UPDATE
        SET secret_ciphertext = EXCLUDED.secret_ciphertext,
            secret_nonce = EXCLUDED.secret_nonce,
            key_version = EXCLUDED.key_version,
            confirmed_at = NULL,
            last_used_step = NULL
        "#,
    )
    .bind(method_id)
    .bind(ciphertext)
    .bind(nonce)
    .bind(version as i16)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("MFA_CHALLENGE_CREATED", user_id);
    event.metadata = json!({"method": "totp", "stage": "setup"});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    let uri = totp.get_url();
    let qr_svg = QrCode::new(uri.as_bytes())
        .map(|code| {
            code.render::<svg::Color<'_>>()
                .min_dimensions(192, 192)
                .build()
        })
        .unwrap_or_default();
    Ok(TotpSetup {
        secret: format_base32(&BASE32_NOPAD.encode(&secret)),
        otpauth_uri: uri,
        qr_svg,
    })
}

pub async fn confirm_totp(
    state: &AppState,
    user_id: Uuid,
    code: &str,
    meta: &ClientMeta,
) -> AppResult<Vec<String>> {
    let code = code.trim();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Err(AppError::Validation("Enter the 6-digit code."));
    }
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, Vec<u8>, Vec<u8>, i16, String)> = sqlx::query_as(
        r#"
        SELECT t.mfa_method_id, t.secret_ciphertext, t.secret_nonce, t.key_version, e.email
        FROM totp_credentials t
        JOIN mfa_methods m ON m.id = t.mfa_method_id
        JOIN user_emails e ON e.user_id = m.user_id AND e.is_primary
        WHERE m.user_id = $1 AND t.confirmed_at IS NULL
        FOR UPDATE OF t
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((method_id, ciphertext, nonce, version, email)) = row else {
        return Err(AppError::Validation("Start authenticator setup again."));
    };
    let secret = state
        .config
        .totp_keys
        .decrypt(version as u16, &nonce, &ciphertext)?;
    let totp = build_totp(&secret, &email)?;
    let step = matching_step(&totp, code).ok_or(AppError::Validation("That code is not valid."))?;
    let now = Utc::now();
    sqlx::query(
        "UPDATE totp_credentials SET confirmed_at = $2, last_used_step = $3 WHERE mfa_method_id = $1",
    )
    .bind(method_id)
    .bind(now)
    .bind(step as i64)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE mfa_methods SET enabled_at = $2, disabled_at = NULL WHERE id = $1")
        .bind(method_id)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    let codes = insert_recovery_codes(&mut tx, user_id).await?;
    let mut event = NewEvent::success("MFA_ENABLED", user_id);
    event.metadata = json!({"method": "totp"});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    let _ = email::enqueue_and_send(
        state,
        &email,
        templates::security_alert(
            "An authenticator app is now enabled on your Knotree account.",
            &format!("{}/account/security", state.config.app_base_url),
        ),
    )
    .await;
    metrics::counter!("auth_mfa_enabled_total", "method" => "totp").increment(1);
    Ok(codes)
}

pub async fn verify_login(
    state: &AppState,
    mfa_token: &str,
    method: &str,
    code: &str,
    meta: &ClientMeta,
) -> AppResult<IssuedSession> {
    let user_id = load_login_user(state, mfa_token).await?;
    rate_limit::enforce(
        state,
        &Limit {
            kind: "mfa_verify",
            subject: format!("user:{user_id}"),
            limit: 8,
            window_seconds: 600,
        },
        meta.ip,
    )
    .await?;
    let mut tx = state.db.begin().await?;
    let row: Option<(Uuid, i32)> = sqlx::query_as(
        r#"
        SELECT id, attempt_count FROM login_transactions
        WHERE token_hash = $1 AND consumed_at IS NULL AND expires_at > now()
        FOR UPDATE
        "#,
    )
    .bind(sha256(mfa_token.as_bytes()))
    .fetch_optional(&mut *tx)
    .await?;
    let Some((tx_id, attempts)) = row else {
        return Err(AppError::Gone("This sign-in attempt expired. Start again."));
    };
    if attempts >= 5 {
        sqlx::query("UPDATE login_transactions SET consumed_at = now() WHERE id = $1")
            .bind(tx_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        return Err(AppError::RateLimited {
            retry_after_seconds: 600,
        });
    }
    let ok = match method {
        "totp" => verify_totp_code(&mut tx, &state.config.totp_keys, user_id, code).await?,
        "email" => verify_email_code(&mut tx, user_id, code).await?,
        "recovery" => verify_recovery_code(&mut tx, user_id, code).await?,
        _ => return Err(AppError::Validation("Choose a valid verification method.")),
    };
    if !ok {
        sqlx::query(
            "UPDATE login_transactions SET attempt_count = attempt_count + 1 WHERE id = $1",
        )
        .bind(tx_id)
        .execute(&mut *tx)
        .await?;
        let mut event = NewEvent {
            event_type: "MFA_FAILED",
            result: "failure",
            actor_user_id: Some(user_id),
            target_user_id: Some(user_id),
            client_id: None,
            session_id: None,
            request_id: None,
            ip: None,
            user_agent: None,
            metadata: json!({"method": method}),
        };
        apply_meta(&mut event, meta);
        super::record(&mut *tx, event).await?;
        tx.commit().await?;
        rate_limit::record(
            state,
            "mfa_verify",
            &format!("user:{user_id}"),
            meta.ip,
            false,
        )
        .await?;
        metrics::counter!("auth_mfa_verify_total", "result" => "failure", "method" => method.to_string()).increment(1);
        return Err(AppError::Validation("That code is not valid."));
    }
    sqlx::query("UPDATE login_transactions SET consumed_at = now() WHERE id = $1")
        .bind(tx_id)
        .execute(&mut *tx)
        .await?;
    let mut event = NewEvent::success("MFA_SUCCESS", user_id);
    event.metadata = json!({"method": method});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    metrics::counter!("auth_mfa_verify_total", "result" => "success", "method" => method.to_string()).increment(1);
    session::issue(
        state,
        IssueParams {
            user_id,
            meta,
            auth_methods: vec!["password".into(), method.into()],
            mfa_satisfied: true,
            elevated: true,
            notify_new_device: true,
        },
    )
    .await
}

pub async fn set_email_mfa(
    state: &AppState,
    user_id: Uuid,
    enabled: bool,
    meta: &ClientMeta,
) -> AppResult<()> {
    let (email, verified) = primary_email(&state.db, user_id)
        .await?
        .ok_or(AppError::Validation("No email is available."))?;
    if enabled && verified.is_none() {
        return Err(AppError::Validation("Verify your email first."));
    }
    let now = Utc::now();
    if enabled {
        sqlx::query(
            r#"
            INSERT INTO mfa_methods (id, user_id, method, created_at, enabled_at)
            VALUES ($1, $2, 'email', $3, $3)
            ON CONFLICT (user_id, method) DO UPDATE SET enabled_at = $3, disabled_at = NULL
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(now)
        .execute(&state.db)
        .await?;
    } else {
        sqlx::query(
            "UPDATE mfa_methods SET disabled_at = now(), enabled_at = NULL WHERE user_id = $1 AND method = 'email'",
        )
        .bind(user_id)
        .execute(&state.db)
        .await?;
    }
    let mut event = NewEvent::success(
        if enabled {
            "MFA_ENABLED"
        } else {
            "MFA_DISABLED"
        },
        user_id,
    );
    event.metadata = json!({"method": "email"});
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    let summary = if enabled {
        "Email verification codes are now enabled on your Knotree account."
    } else {
        "Email verification codes were turned off on your Knotree account."
    };
    let _ = email::enqueue_and_send(
        state,
        &email,
        templates::security_alert(
            summary,
            &format!("{}/account/security", state.config.app_base_url),
        ),
    )
    .await;
    Ok(())
}

pub async fn disable_totp(
    state: &AppState,
    user_id: Uuid,
    code: &str,
    method: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    if session::is_super_admin(state, user_id).await? {
        return Err(AppError::Forbidden(
            "The admin account must keep an authenticator enabled.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let ok = match method {
        "totp" => verify_totp_code(&mut tx, &state.config.totp_keys, user_id, code).await?,
        "recovery" => verify_recovery_code(&mut tx, user_id, code).await?,
        _ => {
            return Err(AppError::Validation(
                "Confirm with an authenticator or recovery code.",
            ))
        }
    };
    if !ok {
        return Err(AppError::Validation("That code is not valid."));
    }
    sqlx::query(
        "UPDATE mfa_methods SET disabled_at = now(), enabled_at = NULL WHERE user_id = $1 AND method = 'totp'",
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
        DELETE FROM totp_credentials
        WHERE mfa_method_id IN (SELECT id FROM mfa_methods WHERE user_id = $1 AND method = 'totp')
        "#,
    )
    .bind(user_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let mut event = NewEvent::success("MFA_DISABLED", user_id);
    event.metadata = json!({"method": "totp"});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    if let Some((email, _)) = primary_email(&state.db, user_id).await? {
        let _ = email::enqueue_and_send(
            state,
            &email,
            templates::security_alert(
                "The authenticator app was turned off on your Knotree account.",
                &format!("{}/account/security", state.config.app_base_url),
            ),
        )
        .await;
    }
    Ok(())
}

pub async fn regenerate_recovery(
    state: &AppState,
    user_id: Uuid,
    meta: &ClientMeta,
) -> AppResult<Vec<String>> {
    if !enabled_mfa_methods(state, user_id)
        .await?
        .iter()
        .any(|m| m == "totp")
    {
        return Err(AppError::Validation(
            "Enable an authenticator before generating recovery codes.",
        ));
    }
    let mut tx = state.db.begin().await?;
    let codes = insert_recovery_codes(&mut tx, user_id).await?;
    let mut event = NewEvent::success("RECOVERY_CODES_REGENERATED", user_id);
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    if let Some((email, _)) = primary_email(&state.db, user_id).await? {
        let _ = email::enqueue_and_send(
            state,
            &email,
            templates::security_alert(
                "Recovery codes for your Knotree account were regenerated. Previous codes no longer work.",
                &format!("{}/account/security", state.config.app_base_url),
            ),
        )
        .await;
    }
    Ok(codes)
}

pub async fn recovery_remaining(state: &AppState, user_id: Uuid) -> AppResult<i64> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM recovery_codes WHERE user_id = $1 AND used_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(&state.db)
    .await?;
    Ok(count)
}

pub async fn security_summary(state: &AppState, user_id: Uuid) -> AppResult<serde_json::Value> {
    let methods = enabled_mfa_methods(state, user_id).await?;
    let remaining = recovery_remaining(state, user_id).await?;
    let password_changed: Option<chrono::DateTime<Utc>> =
        sqlx::query_scalar("SELECT password_changed_at FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&state.db)
            .await?;
    Ok(json!({
        "totp_enabled": methods.iter().any(|m| m == "totp"),
        "email_enabled": methods.iter().any(|m| m == "email"),
        "recovery_codes_remaining": remaining,
        "password_changed_at": password_changed,
    }))
}

async fn insert_recovery_codes(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
) -> AppResult<Vec<String>> {
    sqlx::query("DELETE FROM recovery_codes WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut **tx)
        .await?;
    let batch = Uuid::now_v7();
    let mut codes = Vec::with_capacity(8);
    for _ in 0..8 {
        let code = recovery_code()?;
        sqlx::query(
            r#"
            INSERT INTO recovery_codes (id, user_id, code_hash, batch_id, created_at)
            VALUES ($1,$2,$3,$4,now())
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(sha256(normalize_recovery_code(&code).as_bytes()))
        .bind(batch)
        .execute(&mut **tx)
        .await?;
        codes.push(code);
    }
    Ok(codes)
}

async fn verify_totp_code(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    keys: &TotpKeyring,
    user_id: Uuid,
    code: &str,
) -> AppResult<bool> {
    let code = code.trim();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Ok(false);
    }
    let row: Option<(Uuid, Vec<u8>, Vec<u8>, i16, Option<i64>, String)> = sqlx::query_as(
        r#"
        SELECT t.mfa_method_id, t.secret_ciphertext, t.secret_nonce, t.key_version, t.last_used_step, e.email
        FROM totp_credentials t
        JOIN mfa_methods m ON m.id = t.mfa_method_id
        JOIN user_emails e ON e.user_id = m.user_id AND e.is_primary
        WHERE m.user_id = $1 AND m.enabled_at IS NOT NULL AND m.disabled_at IS NULL AND t.confirmed_at IS NOT NULL
        FOR UPDATE OF t
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((method_id, ciphertext, nonce, version, last_step, email)) = row else {
        return Ok(false);
    };
    let secret = keys.decrypt(version as u16, &nonce, &ciphertext)?;
    let totp = build_totp(&secret, &email)?;
    let Some(step) = matching_step(&totp, code) else {
        return Ok(false);
    };
    if last_step.is_some_and(|used| step <= used as u64) {
        return Ok(false);
    }
    sqlx::query("UPDATE totp_credentials SET last_used_step = $2 WHERE mfa_method_id = $1")
        .bind(method_id)
        .bind(step as i64)
        .execute(&mut **tx)
        .await?;
    Ok(true)
}

async fn verify_email_code(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    code: &str,
) -> AppResult<bool> {
    let code = code.trim();
    if code.len() != 6 || !code.chars().all(|c| c.is_ascii_digit()) {
        return Ok(false);
    }
    let row: Option<(Uuid, Vec<u8>, i32, i32)> = sqlx::query_as(
        r#"
        SELECT id, code_hash, attempt_count, max_attempts
        FROM email_challenges
        WHERE user_id = $1 AND purpose = 'email_mfa' AND consumed_at IS NULL AND expires_at > now()
        ORDER BY created_at DESC
        LIMIT 1
        FOR UPDATE
        "#,
    )
    .bind(user_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((id, hash, attempts, max_attempts)) = row else {
        return Ok(false);
    };
    if attempts >= max_attempts {
        sqlx::query("UPDATE email_challenges SET consumed_at = now() WHERE id = $1")
            .bind(id)
            .execute(&mut **tx)
            .await?;
        return Ok(false);
    }
    if !ct_eq_str_bytes(&hash, &sha256(code.as_bytes())) {
        sqlx::query("UPDATE email_challenges SET attempt_count = attempt_count + 1 WHERE id = $1")
            .bind(id)
            .execute(&mut **tx)
            .await?;
        return Ok(false);
    }
    let updated = sqlx::query(
        "UPDATE email_challenges SET consumed_at = now() WHERE id = $1 AND consumed_at IS NULL",
    )
    .bind(id)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected() == 1)
}

fn ct_eq_str_bytes(left: &[u8], right: &[u8]) -> bool {
    crate::security::ct_eq(left, right)
}

async fn verify_recovery_code(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    code: &str,
) -> AppResult<bool> {
    let normalized = normalize_recovery_code(code);
    if normalized.len() < 12 {
        return Ok(false);
    }
    let hash = sha256(normalized.as_bytes());
    let updated = sqlx::query(
        r#"
        UPDATE recovery_codes
        SET used_at = now()
        WHERE user_id = $1 AND code_hash = $2 AND used_at IS NULL
        "#,
    )
    .bind(user_id)
    .bind(hash)
    .execute(&mut **tx)
    .await?;
    Ok(updated.rows_affected() == 1)
}

fn build_totp(secret: &[u8], email: &str) -> AppResult<TOTP> {
    TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        secret.to_vec(),
        Some("Knotree".into()),
        email.to_string(),
    )
    .map_err(AppError::internal)
}

fn matching_step(totp: &TOTP, code: &str) -> Option<u64> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    let base = now / 30;
    for delta in [-1i64, 0, 1] {
        let step = base as i64 + delta;
        if step < 0 {
            continue;
        }
        let expected = totp.generate((step as u64) * 30);
        if ct_eq_str(&expected, code) {
            return Some(step as u64);
        }
    }
    None
}

fn format_base32(value: &str) -> String {
    value
        .chars()
        .collect::<Vec<_>>()
        .chunks(4)
        .map(|chunk| chunk.iter().collect::<String>())
        .collect::<Vec<_>>()
        .join(" ")
}

async fn session_user_must_be_active(state: &AppState, user_id: Uuid) -> AppResult<()> {
    let status: Option<String> = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&state.db)
        .await?;
    if status.as_deref() != Some("active") {
        return Err(AppError::AccountDisabled);
    }
    Ok(())
}

pub async fn verify_second_factor(
    state: &AppState,
    user_id: Uuid,
    method: &str,
    code: &str,
) -> AppResult<bool> {
    let mut tx = state.db.begin().await?;
    let ok = match method {
        "totp" => verify_totp_code(&mut tx, &state.config.totp_keys, user_id, code).await?,
        "email" => verify_email_code(&mut tx, user_id, code).await?,
        "recovery" => verify_recovery_code(&mut tx, user_id, code).await?,
        _ => return Err(AppError::Validation("Choose a valid verification method.")),
    };
    tx.commit().await?;
    Ok(ok)
}
