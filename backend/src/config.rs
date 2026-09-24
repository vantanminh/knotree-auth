use crate::error::{AppError, AppResult};
use crate::security::crypto::TotpKeyring;
use crate::security::password::{hash_password, ArgonSettings};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rsa::pkcs8::{EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::traits::PublicKeyParts;
use rsa::RsaPrivateKey;
use std::env;
use std::fs;
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Environment {
    Development,
    Staging,
    Production,
}

impl Environment {
    fn parse(value: &str) -> AppResult<Self> {
        match value {
            "development" | "dev" => Ok(Self::Development),
            "staging" => Ok(Self::Staging),
            "production" | "prod" => Ok(Self::Production),
            _ => Err(AppError::internal(
                "APP_ENV must be development, staging, or production",
            )),
        }
    }

    pub fn is_production(self) -> bool {
        matches!(self, Self::Production | Self::Staging)
    }
}

#[derive(Clone)]
pub struct JwtKey {
    pub kid: String,
    pub private_pem: Option<String>,
    pub public_pem: String,
    pub n: String,
    pub e: String,
}

#[derive(Clone)]
pub struct AppConfig {
    pub env: Environment,
    pub database_url: String,
    pub bind: String,
    pub app_base_url: String,
    pub cookie_secure: bool,
    pub session_ttl_hours: i64,
    pub session_idle_hours: i64,
    pub admin_session_hours: i64,
    pub admin_idle_minutes: i64,
    pub step_up_minutes: i64,
    pub argon: ArgonSettings,
    pub dummy_password_hash: String,
    pub totp_keys: TotpKeyring,
    pub jwt_keys: Vec<JwtKey>,
    pub active_kid: String,
    pub email_from: String,
    pub email_provider: String,
    pub resend_api_key: Option<String>,
    pub dev_mailbox: bool,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub github_client_id: Option<String>,
    pub github_client_secret: Option<String>,
    pub google_authorize_url: String,
    pub google_token_url: String,
    pub google_jwks_url: String,
    pub github_authorize_url: String,
    pub github_token_url: String,
    pub github_user_url: String,
    pub github_emails_url: String,
    pub super_admin_user_id: Option<Uuid>,
    pub super_admin_email: Option<String>,
    pub cors_origins: Vec<String>,
    pub trust_proxy: bool,
    pub access_token_seconds: i64,
    pub refresh_token_days: i64,
    pub auth_code_seconds: i64,
    pub email_otp_seconds: i64,
    pub verification_hours: i64,
    pub reset_minutes: i64,
    pub metrics_token: Option<String>,
}

impl AppConfig {
    pub fn issuer(&self) -> &str {
        self.app_base_url.trim_end_matches('/')
    }

    pub fn active_jwt(&self) -> &JwtKey {
        self.jwt_keys
            .iter()
            .find(|key| key.kid == self.active_kid && key.private_pem.is_some())
            .expect("active signing key")
    }
}

pub fn from_env() -> AppResult<AppConfig> {
    let env_name = env_or("APP_ENV", "development");
    let environment = Environment::parse(&env_name)?;
    let database_url = required("DATABASE_URL")?;
    let app_base_url = env_or("APP_BASE_URL", "http://localhost:5173");
    validate_base_url(&app_base_url, environment)?;
    let cookie_secure = match env::var("COOKIE_SECURE") {
        Ok(value) => value == "true" || value == "1",
        Err(_) => environment.is_production() || app_base_url.starts_with("https://"),
    };
    if environment.is_production() && !cookie_secure {
        return Err(AppError::internal(
            "COOKIE_SECURE must be true in production",
        ));
    }
    let dev_mailbox = env_flag("DEV_MAILBOX", environment == Environment::Development);
    if environment.is_production() && dev_mailbox {
        return Err(AppError::internal(
            "DEV_MAILBOX cannot be enabled in production",
        ));
    }
    let argon = ArgonSettings {
        memory_kib: env_u32("ARGON2_MEMORY_KIB", 65_536)?,
        iterations: env_u32("ARGON2_ITERATIONS", 3)?,
        parallelism: env_u32("ARGON2_PARALLELISM", 1)?,
    };
    if environment.is_production() && argon.memory_kib < 19_456 {
        return Err(AppError::internal(
            "ARGON2_MEMORY_KIB must be at least 19456 in production",
        ));
    }
    let totp_keys = load_totp_keys(environment)?;
    let (jwt_keys, active_kid) = load_jwt_keys(environment)?;
    let email_provider = env_or(
        "EMAIL_PROVIDER",
        if dev_mailbox { "outbox" } else { "resend" },
    );
    let resend_api_key = env::var("RESEND_API_KEY").ok().filter(|v| !v.is_empty());
    if email_provider == "resend" && resend_api_key.is_none() && environment.is_production() {
        return Err(AppError::internal(
            "RESEND_API_KEY is required when EMAIL_PROVIDER=resend",
        ));
    }
    let email_from = env_or("EMAIL_FROM", "Knotree Accounts <accounts@knotree.com>");
    let super_admin_user_id = match env::var("SUPER_ADMIN_USER_ID") {
        Ok(value) if !value.trim().is_empty() => Some(
            Uuid::parse_str(value.trim())
                .map_err(|_| AppError::internal("SUPER_ADMIN_USER_ID is not a UUID"))?,
        ),
        _ => None,
    };
    if environment == Environment::Production && super_admin_user_id.is_none() {
        tracing::warn!("SUPER_ADMIN_USER_ID is unset; admin API will stay closed");
    }
    let cors_origins = env::var("CORS_ORIGINS")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .map(|v| v.split(',').map(|s| s.trim().to_string()).collect())
        .unwrap_or_else(|| vec![app_base_url.trim_end_matches('/').to_string()]);

    let dummy_password_hash = hash_password(&format!("dummy-{}", Uuid::now_v7()), &argon)?;

    Ok(AppConfig {
        env: environment,
        database_url,
        bind: env_or("BIND_ADDR", "0.0.0.0:8080"),
        app_base_url: app_base_url.trim_end_matches('/').to_string(),
        cookie_secure,
        session_ttl_hours: env_i64("SESSION_TTL_HOURS", 24 * 14)?,
        session_idle_hours: env_i64("SESSION_IDLE_HOURS", 24 * 7)?,
        admin_session_hours: env_i64("ADMIN_SESSION_HOURS", 12)?,
        admin_idle_minutes: env_i64("ADMIN_IDLE_MINUTES", 60)?,
        step_up_minutes: env_i64("STEP_UP_MINUTES", 10)?,
        argon,
        dummy_password_hash,
        totp_keys,
        jwt_keys,
        active_kid,
        email_from,
        email_provider,
        resend_api_key,
        dev_mailbox,
        google_client_id: empty_none("GOOGLE_CLIENT_ID"),
        google_client_secret: empty_none("GOOGLE_CLIENT_SECRET"),
        github_client_id: empty_none("GITHUB_CLIENT_ID"),
        github_client_secret: empty_none("GITHUB_CLIENT_SECRET"),
        google_authorize_url: env_or(
            "GOOGLE_AUTHORIZE_URL",
            "https://accounts.google.com/o/oauth2/v2/auth",
        ),
        google_token_url: env_or("GOOGLE_TOKEN_URL", "https://oauth2.googleapis.com/token"),
        google_jwks_url: env_or(
            "GOOGLE_JWKS_URL",
            "https://www.googleapis.com/oauth2/v3/certs",
        ),
        github_authorize_url: env_or(
            "GITHUB_AUTHORIZE_URL",
            "https://github.com/login/oauth/authorize",
        ),
        github_token_url: env_or(
            "GITHUB_TOKEN_URL",
            "https://github.com/login/oauth/access_token",
        ),
        github_user_url: env_or("GITHUB_USER_URL", "https://api.github.com/user"),
        github_emails_url: env_or("GITHUB_EMAILS_URL", "https://api.github.com/user/emails"),
        super_admin_user_id,
        super_admin_email: empty_none("SUPER_ADMIN_EMAIL").map(|email| email.to_lowercase()),
        cors_origins,
        trust_proxy: env_flag("TRUST_PROXY", false),
        access_token_seconds: env_i64("ACCESS_TOKEN_SECONDS", 900)?,
        refresh_token_days: env_i64("REFRESH_TOKEN_DAYS", 30)?,
        auth_code_seconds: env_i64("AUTH_CODE_SECONDS", 60)?,
        email_otp_seconds: env_i64("EMAIL_OTP_SECONDS", 600)?,
        verification_hours: env_i64("VERIFICATION_HOURS", 24)?,
        reset_minutes: env_i64("RESET_MINUTES", 30)?,
        metrics_token: empty_none("METRICS_TOKEN"),
    })
}

#[cfg(feature = "test-util")]
pub fn for_tests(database_url: &str) -> AppResult<AppConfig> {
    // SAFETY: tests are single-process and set these before constructing config.
    unsafe {
        env::set_var("APP_ENV", "development");
        env::set_var("DATABASE_URL", database_url);
        env::set_var("APP_BASE_URL", "http://localhost:5173");
        env::set_var("COOKIE_SECURE", "false");
        env::set_var("DEV_MAILBOX", "true");
        env::set_var("EMAIL_PROVIDER", "outbox");
        env::set_var("ARGON2_MEMORY_KIB", "8192");
        env::set_var("ARGON2_ITERATIONS", "1");
        env::set_var("ARGON2_PARALLELISM", "1");
        env::set_var("BIND_ADDR", "127.0.0.1:0");
    }
    from_env()
}

fn load_totp_keys(environment: Environment) -> AppResult<TotpKeyring> {
    if let Ok(spec) = env::var("TOTP_ENCRYPTION_KEYS") {
        if !spec.trim().is_empty() {
            let active = env_u32("TOTP_ENCRYPTION_KEY_VERSION", 1)? as u16;
            return TotpKeyring::from_spec(&spec, active);
        }
    }
    if environment.is_production() {
        return Err(AppError::internal("TOTP_ENCRYPTION_KEYS is required"));
    }
    let dir = dev_key_dir()?;
    let path = dir.join("totp.key");
    if path.exists() {
        let spec = fs::read_to_string(&path).map_err(AppError::internal)?;
        return TotpKeyring::from_spec(spec.trim(), 1);
    }
    let (ring, spec) = TotpKeyring::generate_dev();
    fs::write(&path, &spec).map_err(AppError::internal)?;
    tracing::warn!(path = %path.display(), "generated development TOTP encryption key");
    Ok(ring)
}

fn load_jwt_keys(environment: Environment) -> AppResult<(Vec<JwtKey>, String)> {
    let active_kid = env_or("JWT_KEY_ID", "knotree-1");
    let private_pem = if let Ok(pem) = env::var("JWT_PRIVATE_KEY_PEM") {
        if !pem.trim().is_empty() {
            Some(pem.replace("\\n", "\n"))
        } else {
            None
        }
    } else if let Ok(path) = env::var("JWT_PRIVATE_KEY_FILE") {
        Some(fs::read_to_string(path).map_err(AppError::internal)?)
    } else {
        None
    };
    let private_pem = if let Some(pem) = private_pem {
        pem
    } else if environment.is_production() {
        return Err(AppError::internal("JWT_PRIVATE_KEY_PEM is required"));
    } else {
        let dir = dev_key_dir()?;
        let path = dir.join("jwt.pem");
        if path.exists() {
            fs::read_to_string(&path).map_err(AppError::internal)?
        } else {
            let mut rng = rand::rngs::OsRng;
            let key = RsaPrivateKey::new(&mut rng, 2048).map_err(AppError::internal)?;
            let pem = key
                .to_pkcs8_pem(LineEnding::LF)
                .map_err(AppError::internal)?
                .to_string();
            fs::write(&path, &pem).map_err(AppError::internal)?;
            tracing::warn!(path = %path.display(), "generated development JWT signing key");
            pem
        }
    };
    let mut keys = vec![jwt_key_from_private(&active_kid, &private_pem)?];
    if let Ok(previous) = env::var("JWT_PREVIOUS_PUBLIC_KEY_PEM") {
        if !previous.trim().is_empty() {
            let kid = env_or("JWT_PREVIOUS_KEY_ID", "knotree-0");
            keys.push(jwt_key_from_public(&kid, &previous.replace("\\n", "\n"))?);
        }
    }
    Ok((keys, active_kid))
}

fn jwt_key_from_private(kid: &str, pem: &str) -> AppResult<JwtKey> {
    let private = rsa::pkcs8::DecodePrivateKey::from_pkcs8_pem(pem).map_err(AppError::internal)?;
    let private: RsaPrivateKey = private;
    let public = rsa::RsaPublicKey::from(&private);
    let public_pem = public
        .to_public_key_pem(LineEnding::LF)
        .map_err(AppError::internal)?;
    let (n, e) = rsa_components(&public);
    Ok(JwtKey {
        kid: kid.to_string(),
        private_pem: Some(pem.to_string()),
        public_pem,
        n,
        e,
    })
}

fn jwt_key_from_public(kid: &str, pem: &str) -> AppResult<JwtKey> {
    let public = <rsa::RsaPublicKey as rsa::pkcs8::DecodePublicKey>::from_public_key_pem(pem)
        .map_err(AppError::internal)?;
    let (n, e) = rsa_components(&public);
    Ok(JwtKey {
        kid: kid.to_string(),
        private_pem: None,
        public_pem: pem.to_string(),
        n,
        e,
    })
}

fn rsa_components(public: &rsa::RsaPublicKey) -> (String, String) {
    (
        STANDARD
            .encode(public.n().to_bytes_be())
            .replace('+', "-")
            .replace('/', "_")
            .trim_end_matches('=')
            .to_string(),
        STANDARD
            .encode(public.e().to_bytes_be())
            .replace('+', "-")
            .replace('/', "_")
            .trim_end_matches('=')
            .to_string(),
    )
}

fn dev_key_dir() -> AppResult<PathBuf> {
    let dir = PathBuf::from(env_or("DEV_KEY_DIR", ".dev-keys"));
    fs::create_dir_all(&dir).map_err(AppError::internal)?;
    Ok(dir)
}

fn validate_base_url(url: &str, environment: Environment) -> AppResult<()> {
    let parsed = url::Url::parse(url).map_err(|_| AppError::internal("APP_BASE_URL is invalid"))?;
    if environment.is_production() && parsed.scheme() != "https" {
        return Err(AppError::internal(
            "APP_BASE_URL must be https in production",
        ));
    }
    if parsed.scheme() != "https" && parsed.scheme() != "http" {
        return Err(AppError::internal("APP_BASE_URL scheme is invalid"));
    }
    Ok(())
}

fn required(name: &str) -> AppResult<String> {
    env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| AppError::internal(format!("{name} is required")))
}

fn env_or(name: &str, default: &str) -> String {
    env::var(name)
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn empty_none(name: &str) -> Option<String> {
    env::var(name).ok().filter(|v| !v.trim().is_empty())
}

fn env_flag(name: &str, default: bool) -> bool {
    match env::var(name) {
        Ok(value) => value == "true" || value == "1",
        Err(_) => default,
    }
}

fn env_u32(name: &str, default: u32) -> AppResult<u32> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => value
            .parse()
            .map_err(|_| AppError::internal(format!("{name} must be an integer"))),
        _ => Ok(default),
    }
}

fn env_i64(name: &str, default: i64) -> AppResult<i64> {
    match env::var(name) {
        Ok(value) if !value.is_empty() => value
            .parse()
            .map_err(|_| AppError::internal(format!("{name} must be an integer"))),
        _ => Ok(default),
    }
}
