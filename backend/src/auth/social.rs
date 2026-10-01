use super::session::{self, IssueParams, IssuedSession};
use super::{apply_meta, ClientMeta, NewEvent};
use crate::error::{is_unique_violation, AppError, AppResult};
use crate::security::password::normalize_email;
use crate::security::random::random_token;
use crate::security::redirect::safe_return_to;
use crate::security::sha256;
use crate::state::AppState;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use chrono::{Duration, Utc};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Debug, PartialEq, Eq)]
pub enum SocialLoginDecision {
    SignIn(Uuid),
    Create,
    EmailInUse,
}

pub fn decide_social_login(
    identity_user: Option<Uuid>,
    email_user: Option<Uuid>,
) -> SocialLoginDecision {
    if let Some(user_id) = identity_user {
        return SocialLoginDecision::SignIn(user_id);
    }
    if email_user.is_some() {
        return SocialLoginDecision::EmailInUse;
    }
    SocialLoginDecision::Create
}

pub struct SocialStart {
    pub authorize_url: String,
    pub state_token: String,
}

pub async fn start(
    state: &AppState,
    provider: &str,
    mode: &str,
    user_id: Option<Uuid>,
    return_to: Option<&str>,
) -> AppResult<SocialStart> {
    if !matches!(provider, "google" | "github") {
        return Err(AppError::Validation(
            "This identity provider is not available.",
        ));
    }
    if mode == "link" && user_id.is_none() {
        return Err(AppError::Unauthenticated);
    }
    let (client_id, authorize) = provider_client(state, provider)?;
    let state_token = random_token()?;
    let verifier = random_token()?;
    let nonce = random_token()?;
    let (version, nonce_bytes, ciphertext) = state.config.totp_keys.encrypt(verifier.as_bytes())?;
    let _ = version;
    let return_to = return_to.and_then(safe_return_to);
    let now = Utc::now();
    sqlx::query(
        r#"
        INSERT INTO social_transactions (
            id, provider, state_hash, verifier_ciphertext, verifier_nonce, nonce_hash,
            mode, user_id, return_to, expires_at, created_at
        ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(provider)
    .bind(sha256(state_token.as_bytes()))
    .bind(ciphertext)
    .bind(nonce_bytes)
    .bind(sha256(nonce.as_bytes()))
    .bind(mode)
    .bind(user_id)
    .bind(return_to)
    .bind(now + Duration::minutes(10))
    .bind(now)
    .execute(&state.db)
    .await?;

    let redirect = format!(
        "{}/api/v1/auth/social/{provider}/callback",
        state.config.app_base_url
    );
    let challenge = URL_SAFE_NO_PAD.encode(sha256(verifier.as_bytes()));
    let mut url = url::Url::parse(&authorize).map_err(AppError::internal)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("client_id", &client_id);
        pairs.append_pair("redirect_uri", &redirect);
        pairs.append_pair("state", &state_token);
        pairs.append_pair("code_challenge", &challenge);
        pairs.append_pair("code_challenge_method", "S256");
        match provider {
            "google" => {
                pairs.append_pair("response_type", "code");
                pairs.append_pair("scope", "openid email profile");
                pairs.append_pair("nonce", &nonce);
            }
            "github" => {
                pairs.append_pair("scope", "read:user user:email");
                pairs.append_pair("allow_signup", "true");
            }
            _ => {}
        }
    }
    Ok(SocialStart {
        authorize_url: url.to_string(),
        state_token,
    })
}

pub struct SocialFinish {
    pub session: Option<IssuedSession>,
    pub return_to: Option<String>,
    pub linked: bool,
}

pub async fn finish(
    state: &AppState,
    provider: &str,
    code: &str,
    state_token: &str,
    meta: &ClientMeta,
) -> AppResult<SocialFinish> {
    let mut tx = state.db.begin().await?;
    let row: Option<(
        Uuid,
        Vec<u8>,
        Vec<u8>,
        Option<Vec<u8>>,
        String,
        Option<Uuid>,
        Option<String>,
    )> = sqlx::query_as(
        r#"
        SELECT id, verifier_ciphertext, verifier_nonce, nonce_hash, mode, user_id, return_to
        FROM social_transactions
        WHERE provider = $1 AND state_hash = $2 AND consumed_at IS NULL AND expires_at > now()
        FOR UPDATE
        "#,
    )
    .bind(provider)
    .bind(sha256(state_token.as_bytes()))
    .fetch_optional(&mut *tx)
    .await?;
    let Some((id, ciphertext, nonce_bytes, nonce_hash, mode, link_user, return_to)) = row else {
        return Err(AppError::Validation(
            "This sign-in attempt expired. Start again.",
        ));
    };
    sqlx::query("UPDATE social_transactions SET consumed_at = now() WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    let verifier_bytes = state.config.totp_keys.decrypt(
        state.config.totp_keys.active_version(),
        &nonce_bytes,
        &ciphertext,
    )?;
    let verifier = String::from_utf8(verifier_bytes).map_err(AppError::internal)?;
    let profile = match provider {
        "google" => google_profile(state, code, &verifier, nonce_hash.as_deref()).await?,
        "github" => github_profile(state, code, &verifier).await?,
        _ => {
            return Err(AppError::Validation(
                "This identity provider is not available.",
            ))
        }
    };
    if mode == "link" {
        let user_id = link_user.ok_or(AppError::Unauthenticated)?;
        link_identity(state, user_id, &profile, meta).await?;
        return Ok(SocialFinish {
            session: None,
            return_to,
            linked: true,
        });
    }
    let identity_user: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM identities WHERE provider = $1 AND provider_subject = $2",
    )
    .bind(profile.provider)
    .bind(&profile.subject)
    .fetch_optional(&state.db)
    .await?;
    let email_user = if profile.email_verified {
        if let Some(email) = &profile.email {
            sqlx::query_scalar::<_, Uuid>("SELECT user_id FROM user_emails WHERE email = $1")
                .bind(email)
                .fetch_optional(&state.db)
                .await?
        } else {
            None
        }
    } else {
        None
    };
    let user_id = match decide_social_login(identity_user, email_user) {
        SocialLoginDecision::SignIn(user_id) => user_id,
        SocialLoginDecision::EmailInUse => {
            return Err(AppError::Conflict(
                "An account already uses this email. Sign in and connect the provider from security settings.",
            ));
        }
        SocialLoginDecision::Create => create_social_user(state, &profile, meta).await?,
    };
    let status: String = sqlx::query_scalar("SELECT status FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    if status != "active" {
        return Err(AppError::AccountDisabled);
    }
    sqlx::query("UPDATE identities SET last_used_at = now() WHERE user_id = $1 AND provider = $2")
        .bind(user_id)
        .bind(profile.provider)
        .execute(&state.db)
        .await?;
    let issued = session::issue(
        state,
        IssueParams {
            user_id,
            meta,
            auth_methods: vec![provider.into()],
            mfa_satisfied: false,
            elevated: true,
            notify_new_device: true,
        },
    )
    .await?;
    Ok(SocialFinish {
        session: Some(issued),
        return_to,
        linked: false,
    })
}

pub async fn unlink(
    state: &AppState,
    user_id: Uuid,
    provider: &str,
    meta: &ClientMeta,
) -> AppResult<()> {
    if !matches!(provider, "google" | "github") {
        return Err(AppError::Validation(
            "This identity provider is not available.",
        ));
    }
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM identities WHERE user_id = $1")
        .bind(user_id)
        .fetch_one(&state.db)
        .await?;
    if count <= 1 {
        return Err(AppError::Validation(
            "Add a password or another provider before disconnecting this one.",
        ));
    }
    let deleted = sqlx::query("DELETE FROM identities WHERE user_id = $1 AND provider = $2")
        .bind(user_id)
        .bind(provider)
        .execute(&state.db)
        .await?;
    if deleted.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    let mut event = NewEvent::success("IDENTITY_UNLINKED", user_id);
    event.metadata = json!({"provider": provider});
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    Ok(())
}

struct ProviderProfile {
    provider: &'static str,
    subject: String,
    email: Option<String>,
    email_verified: bool,
    name: Option<String>,
}

fn provider_client(state: &AppState, provider: &str) -> AppResult<(String, String)> {
    match provider {
        "google" => {
            let id = state
                .config
                .google_client_id
                .clone()
                .ok_or(AppError::Validation("Google sign-in is not configured."))?;
            Ok((id, state.config.google_authorize_url.clone()))
        }
        "github" => {
            let id = state
                .config
                .github_client_id
                .clone()
                .ok_or(AppError::Validation("GitHub sign-in is not configured."))?;
            Ok((id, state.config.github_authorize_url.clone()))
        }
        _ => Err(AppError::Validation(
            "This identity provider is not available.",
        )),
    }
}

async fn google_profile(
    state: &AppState,
    code: &str,
    verifier: &str,
    nonce_hash: Option<&[u8]>,
) -> AppResult<ProviderProfile> {
    let client_id = state
        .config
        .google_client_id
        .clone()
        .ok_or(AppError::Validation("Google sign-in is not configured."))?;
    let client_secret = state
        .config
        .google_client_secret
        .clone()
        .ok_or(AppError::Validation("Google sign-in is not configured."))?;
    let redirect = format!(
        "{}/api/v1/auth/social/google/callback",
        state.config.app_base_url
    );
    let response = state
        .http
        .post(&state.config.google_token_url)
        .form(&[
            ("code", code),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("grant_type", "authorization_code"),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(AppError::internal)?;
    if !response.status().is_success() {
        return Err(AppError::Validation("Google did not accept this sign-in."));
    }
    let body: Value = response.json().await.map_err(AppError::internal)?;
    let id_token = body
        .get("id_token")
        .and_then(|v| v.as_str())
        .ok_or(AppError::Validation(
            "Google did not return an identity token.",
        ))?;
    let header = decode_header(id_token)
        .map_err(|_| AppError::Validation("Google identity token is invalid."))?;
    let kid = header
        .kid
        .ok_or(AppError::Validation("Google identity token is invalid."))?;
    let jwks: GoogleJwks = state
        .http
        .get(&state.config.google_jwks_url)
        .send()
        .await
        .map_err(AppError::internal)?
        .json()
        .await
        .map_err(AppError::internal)?;
    let jwk = jwks
        .keys
        .into_iter()
        .find(|key| key.kid == kid)
        .ok_or(AppError::Validation("Google identity token is invalid."))?;
    let key = DecodingKey::from_rsa_components(&jwk.n, &jwk.e)
        .map_err(|_| AppError::Validation("Google identity token is invalid."))?;
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_audience(&[&client_id]);
    validation.set_issuer(&["https://accounts.google.com", "accounts.google.com"]);
    let data = decode::<GoogleClaims>(id_token, &key, &validation)
        .map_err(|_| AppError::Validation("Google identity token is invalid."))?;
    if let Some(expected) = nonce_hash {
        let nonce = data.claims.nonce.unwrap_or_default();
        if sha256(nonce.as_bytes()) != expected {
            return Err(AppError::Validation("Google identity token is invalid."));
        }
    }
    let email = data
        .claims
        .email
        .as_deref()
        .map(normalize_email)
        .transpose()?;
    Ok(ProviderProfile {
        provider: "google",
        subject: data.claims.sub,
        email,
        email_verified: data.claims.email_verified.unwrap_or(false),
        name: data.claims.name,
    })
}

async fn github_profile(
    state: &AppState,
    code: &str,
    verifier: &str,
) -> AppResult<ProviderProfile> {
    let client_id = state
        .config
        .github_client_id
        .clone()
        .ok_or(AppError::Validation("GitHub sign-in is not configured."))?;
    let client_secret = state
        .config
        .github_client_secret
        .clone()
        .ok_or(AppError::Validation("GitHub sign-in is not configured."))?;
    let redirect = format!(
        "{}/api/v1/auth/social/github/callback",
        state.config.app_base_url
    );
    let token_response = state
        .http
        .post(&state.config.github_token_url)
        .header("accept", "application/json")
        .form(&[
            ("code", code),
            ("client_id", client_id.as_str()),
            ("client_secret", client_secret.as_str()),
            ("redirect_uri", redirect.as_str()),
            ("code_verifier", verifier),
        ])
        .send()
        .await
        .map_err(AppError::internal)?;
    if !token_response.status().is_success() {
        return Err(AppError::Validation("GitHub did not accept this sign-in."));
    }
    let token_body: Value = token_response.json().await.map_err(AppError::internal)?;
    let access = token_body
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or(AppError::Validation("GitHub did not accept this sign-in."))?;
    let user: GithubUser = state
        .http
        .get(&state.config.github_user_url)
        .bearer_auth(access)
        .header("user-agent", "KnotreeAccounts")
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(AppError::internal)?
        .json()
        .await
        .map_err(AppError::internal)?;
    let emails: Vec<GithubEmail> = state
        .http
        .get(&state.config.github_emails_url)
        .bearer_auth(access)
        .header("user-agent", "KnotreeAccounts")
        .header("accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(AppError::internal)?
        .json()
        .await
        .map_err(AppError::internal)?;
    let primary = emails
        .into_iter()
        .find(|email| email.primary && email.verified);
    let Some(primary) = primary else {
        return Err(AppError::Validation(
            "GitHub did not provide a verified email.",
        ));
    };
    Ok(ProviderProfile {
        provider: "github",
        subject: user.id.to_string(),
        email: Some(normalize_email(&primary.email)?),
        email_verified: true,
        name: user.name.or(user.login),
    })
}

async fn link_identity(
    state: &AppState,
    user_id: Uuid,
    profile: &ProviderProfile,
    meta: &ClientMeta,
) -> AppResult<()> {
    let existing: Option<Uuid> = sqlx::query_scalar(
        "SELECT user_id FROM identities WHERE provider = $1 AND provider_subject = $2",
    )
    .bind(profile.provider)
    .bind(&profile.subject)
    .fetch_optional(&state.db)
    .await?;
    if let Some(owner) = existing {
        if owner != user_id {
            return Err(AppError::Conflict(
                "That provider account is already connected to another user.",
            ));
        }
        return Ok(());
    }
    let inserted = sqlx::query(
        r#"
        INSERT INTO identities (id, user_id, provider, provider_subject, email, email_verified, created_at)
        VALUES ($1,$2,$3,$4,$5,$6,now())
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(profile.provider)
    .bind(&profile.subject)
    .bind(&profile.email)
    .bind(profile.email_verified)
    .execute(&state.db)
    .await;
    if let Err(err) = inserted {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict(
                "That provider account is already connected.",
            ));
        }
        return Err(err.into());
    }
    let mut event = NewEvent::success("IDENTITY_LINKED", user_id);
    event.metadata = json!({"provider": profile.provider});
    apply_meta(&mut event, meta);
    super::record(&state.db, event).await?;
    Ok(())
}

async fn create_social_user(
    state: &AppState,
    profile: &ProviderProfile,
    meta: &ClientMeta,
) -> AppResult<Uuid> {
    if !profile.email_verified {
        return Err(AppError::Validation(
            "The provider did not verify this email.",
        ));
    }
    let email = profile
        .email
        .clone()
        .ok_or(AppError::Validation("The provider did not share an email."))?;
    let now = Utc::now();
    let user_id = Uuid::now_v7();
    let mut tx = state.db.begin().await?;
    let display_name = profile
        .name
        .as_ref()
        .map(|name| name.chars().take(80).collect::<String>());
    sqlx::query("INSERT INTO users (id, display_name, status, created_at, updated_at, locale) VALUES ($1,$2,'active',$3,$3,$4)")
        .bind(user_id)
        .bind(&display_name)
        .bind(now)
        .bind(meta.locale.as_str())
        .execute(&mut *tx)
        .await?;
    if let Err(err) = sqlx::query(
        "INSERT INTO user_emails (id, user_id, email, is_primary, verified_at, created_at) VALUES ($1,$2,$3,TRUE,$4,$4)",
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(&email)
    .bind(now)
    .execute(&mut *tx)
    .await
    {
        if is_unique_violation(&err) {
            return Err(AppError::Conflict(
                "An account already uses this email. Sign in and connect the provider from security settings.",
            ));
        }
        return Err(err.into());
    }
    sqlx::query(
        r#"
        INSERT INTO identities (id, user_id, provider, provider_subject, email, email_verified, created_at)
        VALUES ($1,$2,$3,$4,$5,TRUE,$6)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(user_id)
    .bind(profile.provider)
    .bind(&profile.subject)
    .bind(&email)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    let mut event = NewEvent::success("USER_REGISTERED", user_id);
    event.metadata = json!({"provider": profile.provider});
    apply_meta(&mut event, meta);
    super::record(&mut *tx, event).await?;
    tx.commit().await?;
    Ok(user_id)
}

#[derive(Deserialize)]
struct GoogleJwks {
    keys: Vec<GoogleJwk>,
}

#[derive(Deserialize)]
struct GoogleJwk {
    kid: String,
    n: String,
    e: String,
}

#[derive(Deserialize)]
struct GoogleClaims {
    sub: String,
    email: Option<String>,
    email_verified: Option<bool>,
    name: Option<String>,
    nonce: Option<String>,
}

#[derive(Deserialize)]
struct GithubUser {
    id: i64,
    login: Option<String>,
    name: Option<String>,
}

#[derive(Deserialize)]
struct GithubEmail {
    email: String,
    primary: bool,
    verified: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn does_not_merge_on_email_alone() {
        let existing = Uuid::now_v7();
        assert_eq!(
            decide_social_login(None, Some(existing)),
            SocialLoginDecision::EmailInUse
        );
        assert_eq!(
            decide_social_login(Some(existing), Some(existing)),
            SocialLoginDecision::SignIn(existing)
        );
        assert_eq!(decide_social_login(None, None), SocialLoginDecision::Create);
    }
}
