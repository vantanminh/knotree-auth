use crate::error::{AppError, AppResult};
use crate::security::crypto::TotpKeyring;
use crate::security::password::{hash_password, ArgonSettings};
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use rsa::pkcs8::{
    DecodePrivateKey, DecodePublicKey, EncodePrivateKey, EncodePublicKey, LineEnding,
};
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
    pub email_from_name: Option<String>,
    pub email_provider: String,
    pub cloudflare_account_id: Option<String>,
    pub cloudflare_email_api_token: Option<String>,
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
    /// With `super_admin_email`, creates the super admin account at startup
    /// when it does not exist yet. An existing password is never overwritten.
    pub super_admin_password: Option<String>,
    pub super_admin_username: String,
    /// Directory where uploaded service (OAuth client) logos are stored.
    pub client_logo_dir: std::path::PathBuf,
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
    let problems = env_problems();
    if !problems.is_empty() {
        return Err(AppError::internal(format!(
            "environment check failed:\n{}",
            problems.join("\n")
        )));
    }
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
        if dev_mailbox { "outbox" } else { "cloudflare" },
    );
    if !matches!(email_provider.as_str(), "outbox" | "cloudflare") {
        return Err(AppError::internal(
            "EMAIL_PROVIDER must be outbox or cloudflare",
        ));
    }
    let cloudflare_account_id = env::var("CLOUDFLARE_ACCOUNT_ID")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    let cloudflare_email_api_token = env::var("CLOUDFLARE_EMAIL_API_TOKEN")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if email_provider == "cloudflare" {
        if cloudflare_account_id
            .as_ref()
            .is_some_and(|id| id.len() != 32 || !id.bytes().all(|byte| byte.is_ascii_hexdigit()))
        {
            return Err(AppError::internal(
                "CLOUDFLARE_ACCOUNT_ID must be a 32-character hexadecimal ID",
            ));
        }
        if environment.is_production() && cloudflare_account_id.is_none() {
            return Err(AppError::internal(
                "CLOUDFLARE_ACCOUNT_ID is required when EMAIL_PROVIDER=cloudflare",
            ));
        }
        if environment.is_production() && cloudflare_email_api_token.is_none() {
            return Err(AppError::internal(
                "CLOUDFLARE_EMAIL_API_TOKEN is required when EMAIL_PROVIDER=cloudflare",
            ));
        }
    }
    let email_from = env_or("EMAIL_FROM", "accounts@knotree.com");
    let email_from_name = empty_none("EMAIL_FROM_NAME");
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
        email_from_name,
        email_provider,
        cloudflare_account_id,
        cloudflare_email_api_token,
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
        super_admin_password: empty_none("SUPER_ADMIN_PASSWORD"),
        super_admin_username: env_or("SUPER_ADMIN_USERNAME", "admin"),
        client_logo_dir: env_or("CLIENT_LOGO_DIR", "./data/client-logos").into(),
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
            Some(pem_text(&pem))
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
            keys.push(jwt_key_from_public(&kid, &pem_text(&previous))?);
        }
    }
    Ok((keys, active_kid))
}

fn jwt_key_from_private(kid: &str, pem: &str) -> AppResult<JwtKey> {
    let private =
        <RsaPrivateKey as DecodePrivateKey>::from_pkcs8_pem(pem).map_err(AppError::internal)?;
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
    let public = <rsa::RsaPublicKey as DecodePublicKey>::from_public_key_pem(pem)
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

/// Every set variable is checked. Production also requires the settings the
/// process cannot start without. Callers get the full list, not the first failure.
pub fn env_problems() -> Vec<String> {
    let vars: std::collections::HashMap<String, String> = env::vars().collect();
    check_assignments(&vars)
}

pub fn check_assignments(vars: &std::collections::HashMap<String, String>) -> Vec<String> {
    let mut problems = Vec::new();
    let env_name = vars
        .get("APP_ENV")
        .filter(|value| !value.is_empty())
        .map(|value| value.as_str())
        .unwrap_or("development");
    let environment = match Environment::parse(env_name) {
        Ok(environment) => Some(environment),
        Err(_) => {
            problems.push("APP_ENV must be development, staging, or production".into());
            None
        }
    };
    let production = environment.is_some_and(Environment::is_production);

    match vars.get("DATABASE_URL").map(|value| value.trim()) {
        Some(url) if !url.is_empty() => {
            if url::Url::parse(url)
                .ok()
                .filter(|parsed| matches!(parsed.scheme(), "postgres" | "postgresql"))
                .is_none()
            {
                problems.push("DATABASE_URL must be a postgres or postgresql URL".into());
            }
        }
        _ => problems.push("DATABASE_URL is required".into()),
    }

    if let Some(bind) = vars.get("BIND_ADDR").filter(|value| !value.is_empty()) {
        if bind.parse::<std::net::SocketAddr>().is_err() {
            problems.push("BIND_ADDR must be host:port".into());
        }
    }

    let base = vars
        .get("APP_BASE_URL")
        .filter(|value| !value.is_empty())
        .map(|value| value.as_str())
        .unwrap_or("http://localhost:5173");
    match url::Url::parse(base) {
        Ok(parsed) if parsed.scheme() == "https" || parsed.scheme() == "http" => {
            if production && parsed.scheme() != "https" {
                problems.push("APP_BASE_URL must be https in production".into());
            }
        }
        _ => problems.push("APP_BASE_URL is invalid".into()),
    }

    if let Some(value) = vars.get("COOKIE_SECURE").filter(|value| !value.is_empty()) {
        if !matches!(value.as_str(), "true" | "false" | "1" | "0") {
            problems.push("COOKIE_SECURE must be true or false".into());
        } else if production && value != "true" && value != "1" {
            problems.push("COOKIE_SECURE must be true in production".into());
        }
    }

    if let Some(value) = vars.get("DEV_MAILBOX").filter(|value| !value.is_empty()) {
        if !matches!(value.as_str(), "true" | "false" | "1" | "0") {
            problems.push("DEV_MAILBOX must be true or false".into());
        } else if production && (value == "true" || value == "1") {
            problems.push("DEV_MAILBOX cannot be enabled in production".into());
        }
    }

    if let Some(value) = vars.get("TRUST_PROXY").filter(|value| !value.is_empty()) {
        if !matches!(value.as_str(), "true" | "false" | "1" | "0") {
            problems.push("TRUST_PROXY must be true or false".into());
        }
    }

    let dev_mailbox = vars
        .get("DEV_MAILBOX")
        .map(|value| value == "true" || value == "1")
        .unwrap_or(environment == Some(Environment::Development));
    let email_provider = vars
        .get("EMAIL_PROVIDER")
        .filter(|value| !value.is_empty())
        .cloned()
        .unwrap_or_else(|| {
            if dev_mailbox {
                "outbox".into()
            } else {
                "cloudflare".into()
            }
        });
    if !matches!(email_provider.as_str(), "outbox" | "cloudflare") {
        problems.push("EMAIL_PROVIDER must be outbox or cloudflare".into());
    }
    if let Some(from) = vars
        .get("EMAIL_FROM")
        .filter(|value| !value.trim().is_empty())
    {
        if !from.contains('@') || from.contains(char::is_whitespace) {
            problems.push("EMAIL_FROM must be an email address".into());
        }
    }
    if email_provider == "cloudflare" {
        match vars
            .get("CLOUDFLARE_ACCOUNT_ID")
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
        {
            Some(id) if id.len() == 32 && id.bytes().all(|byte| byte.is_ascii_hexdigit()) => {}
            Some(_) => {
                problems.push("CLOUDFLARE_ACCOUNT_ID must be a 32-character hexadecimal ID".into())
            }
            None if production => problems
                .push("CLOUDFLARE_ACCOUNT_ID is required when EMAIL_PROVIDER=cloudflare".into()),
            None => {}
        }
        if production
            && vars
                .get("CLOUDFLARE_EMAIL_API_TOKEN")
                .map(|value| value.trim())
                .filter(|value| !value.is_empty())
                .is_none()
        {
            problems.push(
                "CLOUDFLARE_EMAIL_API_TOKEN is required when EMAIL_PROVIDER=cloudflare".into(),
            );
        }
    }

    if let Some(value) = vars
        .get("ARGON2_MEMORY_KIB")
        .filter(|value| !value.is_empty())
    {
        match value.parse::<u32>() {
            Ok(memory) if production && memory < 19_456 => {
                problems.push("ARGON2_MEMORY_KIB must be at least 19456 in production".into());
            }
            Ok(_) => {}
            Err(_) => problems.push("ARGON2_MEMORY_KIB must be an integer".into()),
        }
    }
    for name in ["ARGON2_ITERATIONS", "ARGON2_PARALLELISM"] {
        if let Some(value) = vars.get(name).filter(|value| !value.is_empty()) {
            if value.parse::<u32>().ok().filter(|n| *n > 0).is_none() {
                problems.push(format!("{name} must be a positive integer"));
            }
        }
    }

    let active_totp = vars
        .get("TOTP_ENCRYPTION_KEY_VERSION")
        .filter(|value| !value.is_empty())
        .map(|value| value.as_str())
        .unwrap_or("1");
    let active_totp_version = match active_totp.parse::<u16>() {
        Ok(version) => version,
        Err(_) => {
            problems.push("TOTP_ENCRYPTION_KEY_VERSION must be an integer".into());
            1
        }
    };
    match vars
        .get("TOTP_ENCRYPTION_KEYS")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some(spec) => {
            if let Err(err) = TotpKeyring::from_spec(spec, active_totp_version) {
                problems.push(format!("TOTP_ENCRYPTION_KEYS: {}", err.startup_message()));
            }
        }
        None if production => problems.push("TOTP_ENCRYPTION_KEYS is required".into()),
        None => {}
    }

    match vars
        .get("JWT_PRIVATE_KEY_PEM")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        Some(pem) => check_private_pem("JWT_PRIVATE_KEY_PEM", pem, &mut problems),
        None if production => problems.push("JWT_PRIVATE_KEY_PEM is required".into()),
        None => {}
    }
    if let Some(pem) = vars
        .get("JWT_PREVIOUS_PUBLIC_KEY_PEM")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        check_public_pem("JWT_PREVIOUS_PUBLIC_KEY_PEM", pem, &mut problems);
    }

    if let Some(origins) = vars
        .get("CORS_ORIGINS")
        .filter(|value| !value.trim().is_empty())
    {
        for origin in origins
            .split(',')
            .map(str::trim)
            .filter(|origin| !origin.is_empty())
        {
            let parsed = url::Url::parse(origin).ok();
            if parsed
                .as_ref()
                .filter(|url| matches!(url.scheme(), "http" | "https"))
                .is_none()
            {
                problems.push(format!("CORS_ORIGINS contains an invalid URL: {origin}"));
            }
        }
    }

    if let Some(value) = vars
        .get("SUPER_ADMIN_USER_ID")
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    {
        if Uuid::parse_str(value).is_err() {
            problems.push("SUPER_ADMIN_USER_ID is not a UUID".into());
        }
    }
    for (left, right) in [
        ("GOOGLE_CLIENT_ID", "GOOGLE_CLIENT_SECRET"),
        ("GITHUB_CLIENT_ID", "GITHUB_CLIENT_SECRET"),
    ] {
        let has_left = vars
            .get(left)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        let has_right = vars
            .get(right)
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);
        if has_left != has_right {
            problems.push(format!(
                "{left} and {right} must both be set or both be empty"
            ));
        }
    }
    for name in [
        "SESSION_TTL_HOURS",
        "SESSION_IDLE_HOURS",
        "ADMIN_SESSION_HOURS",
        "ADMIN_IDLE_MINUTES",
        "STEP_UP_MINUTES",
        "ACCESS_TOKEN_SECONDS",
        "REFRESH_TOKEN_DAYS",
        "AUTH_CODE_SECONDS",
        "EMAIL_OTP_SECONDS",
        "VERIFICATION_HOURS",
        "RESET_MINUTES",
    ] {
        if let Some(value) = vars.get(name).filter(|value| !value.is_empty()) {
            if value.parse::<i64>().ok().filter(|n| *n > 0).is_none() {
                problems.push(format!("{name} must be a positive integer"));
            }
        }
    }
    problems
}

fn restore_pem_plus(pem: &str) -> String {
    let Some(header_start) = pem.find("-----BEGIN ") else {
        return pem.to_string();
    };
    let after_label = header_start + "-----BEGIN ".len();
    let Some(label_rel) = pem[after_label..].find("-----") else {
        return pem.to_string();
    };
    let header_end = after_label + label_rel + 5;
    let Some(end_rel) = pem[header_end..].find("-----END ") else {
        return format!(
            "{}{}",
            &pem[..header_end],
            pem[header_end..].replace(' ', "+")
        );
    };
    let end = header_end + end_rel;
    format!(
        "{}{}{}",
        &pem[..header_end],
        pem[header_end..end].replace(' ', "+"),
        &pem[end..]
    )
}

fn pem_text(raw: &str) -> String {
    let trimmed = raw.trim().trim_matches(|c| c == '"' || c == '\'');
    let unescaped = trimmed.replace("\\n", "\n").replace('\r', "");
    if unescaped.contains("BEGIN") {
        return restore_pem_plus(&unescaped);
    }
    if let Ok(bytes) = crate::security::crypto::decode_base64_flexible(trimmed) {
        if let Ok(text) = String::from_utf8(bytes) {
            if text.contains("BEGIN") {
                return text;
            }
        }
    }
    unescaped
}

fn check_private_pem(name: &str, raw: &str, problems: &mut Vec<String>) {
    if raw.contains('\0') {
        problems.push(format!("{name} contains a NUL byte"));
        return;
    }
    let pem = pem_text(raw);
    if <RsaPrivateKey as DecodePrivateKey>::from_pkcs8_pem(&pem).is_err() {
        problems.push(format!("{name} is not a PKCS#8 private key"));
    }
}

fn check_public_pem(name: &str, raw: &str, problems: &mut Vec<String>) {
    if raw.contains('\0') {
        problems.push(format!("{name} contains a NUL byte"));
        return;
    }
    let pem = pem_text(raw);
    if <rsa::RsaPublicKey as DecodePublicKey>::from_public_key_pem(&pem).is_err() {
        problems.push(format!("{name} is not a PKCS#8 public key"));
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::RsaPrivateKey;

    fn production_vars() -> std::collections::HashMap<String, String> {
        let mut rng = rand::rngs::OsRng;
        let key = RsaPrivateKey::new(&mut rng, 2048).unwrap();
        let pem = key.to_pkcs8_pem(LineEnding::LF).unwrap().to_string();
        let mut vars = std::collections::HashMap::new();
        vars.insert("APP_ENV".into(), "production".into());
        vars.insert(
            "DATABASE_URL".into(),
            "postgresql://postgres:secret@db:5432/auth".into(),
        );
        vars.insert("APP_BASE_URL".into(), "https://accounts.knotree.com".into());
        vars.insert("COOKIE_SECURE".into(), "true".into());
        vars.insert("DEV_MAILBOX".into(), "false".into());
        vars.insert("EMAIL_PROVIDER".into(), "cloudflare".into());
        vars.insert(
            "CLOUDFLARE_ACCOUNT_ID".into(),
            "660453f3bb001791035317c5afd375f0".into(),
        );
        vars.insert("CLOUDFLARE_EMAIL_API_TOKEN".into(), "token".into());
        vars.insert("ARGON2_MEMORY_KIB".into(), "19456".into());
        vars.insert(
            "TOTP_ENCRYPTION_KEYS".into(),
            format!("1:{}", STANDARD.encode([7u8; 32])),
        );
        vars.insert("JWT_PRIVATE_KEY_PEM".into(), pem.replace('\n', "\\n"));
        vars
    }

    #[test]
    fn production_assignments_pass() {
        let problems = check_assignments(&production_vars());
        assert!(problems.is_empty(), "{problems:?}");
    }

    #[test]
    fn production_reports_every_invalid_secret() {
        let mut vars = production_vars();
        vars.insert("TOTP_ENCRYPTION_KEYS".into(), "1:not base64!!!".into());
        let mut pem = vars.get("JWT_PRIVATE_KEY_PEM").unwrap().clone();
        pem.insert(0, '\0');
        vars.insert("JWT_PRIVATE_KEY_PEM".into(), pem);
        vars.insert("CLOUDFLARE_ACCOUNT_ID".into(), "short".into());
        let problems = check_assignments(&vars);
        let joined = problems.join("\n");
        assert!(joined.contains("TOTP_ENCRYPTION_KEYS"), "{joined}");
        assert!(joined.contains("NUL byte"), "{joined}");
        assert!(joined.contains("CLOUDFLARE_ACCOUNT_ID"), "{joined}");
    }
}
