use crate::auth::{self, ClientMeta, CurrentSession};
use crate::config::AppConfig;
use crate::error::{AppError, REQUEST_ID};
use crate::security::ct_eq_str;
use crate::security::random::random_token;
use crate::state::AppState;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::HeaderMap;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cookie::time::Duration as CookieDuration;
use ipnetwork::IpNetwork;
use std::net::{IpAddr, SocketAddr};

pub fn session_cookie_name(config: &AppConfig) -> &'static str {
    if config.cookie_secure {
        "__Host-knotree_session"
    } else {
        "knotree_session"
    }
}

pub fn csrf_cookie_name(config: &AppConfig) -> &'static str {
    if config.cookie_secure {
        "__Host-knotree_csrf"
    } else {
        "knotree_csrf"
    }
}

pub fn social_cookie_name(config: &AppConfig) -> &'static str {
    if config.cookie_secure {
        "__Host-knotree_social"
    } else {
        "knotree_social"
    }
}

pub fn build_cookie(
    config: &AppConfig,
    name: &'static str,
    value: &str,
    http_only: bool,
    max_age_secs: i64,
) -> Cookie<'static> {
    Cookie::build((name, value.to_owned()))
        .path("/")
        .http_only(http_only)
        .secure(config.cookie_secure)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(max_age_secs))
        .build()
}

pub fn clear_cookie(config: &AppConfig, name: &'static str) -> Cookie<'static> {
    Cookie::build((name, ""))
        .path("/")
        .http_only(true)
        .secure(config.cookie_secure)
        .same_site(SameSite::Lax)
        .max_age(CookieDuration::seconds(0))
        .build()
}

pub fn issue_csrf(config: &AppConfig, jar: CookieJar) -> Result<(CookieJar, String), AppError> {
    let existing = jar
        .get(csrf_cookie_name(config))
        .map(|cookie| cookie.value().to_string());
    if let Some(token) = existing {
        if token.len() >= 20 {
            return Ok((jar, token));
        }
    }
    let token = random_token()?;
    let cookie = build_cookie(
        config,
        csrf_cookie_name(config),
        &token,
        false,
        60 * 60 * 24,
    );
    Ok((jar.add(cookie), token))
}

pub struct Meta(pub ClientMeta);

impl FromRequestParts<AppState> for Meta {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let request_id = REQUEST_ID
            .try_with(|id| id.clone())
            .unwrap_or_else(|_| "req_unknown".to_string());
        let ip = client_ip(
            &parts.headers,
            parts
                .extensions
                .get::<axum::extract::ConnectInfo<SocketAddr>>()
                .map(|info| info.0),
            state.config.trust_proxy,
        );
        let user_agent = parts
            .headers
            .get("user-agent")
            .and_then(|value| value.to_str().ok())
            .map(|value| value.chars().take(512).collect());
        Ok(Meta(ClientMeta {
            request_id,
            ip,
            user_agent,
        }))
    }
}

fn client_ip(
    headers: &HeaderMap,
    peer: Option<SocketAddr>,
    trust_proxy: bool,
) -> Option<IpNetwork> {
    if trust_proxy {
        if let Some(value) = headers
            .get("cf-connecting-ip")
            .and_then(|value| value.to_str().ok())
        {
            if let Some(ip) = parse_ip(value) {
                return Some(ip);
            }
        }
        if let Some(value) = headers
            .get("x-forwarded-for")
            .and_then(|value| value.to_str().ok())
        {
            if let Some(first) = value.split(',').next() {
                if let Some(ip) = parse_ip(first) {
                    return Some(ip);
                }
            }
        }
    }
    peer.map(|addr| IpNetwork::from(addr.ip()))
}

fn parse_ip(value: &str) -> Option<IpNetwork> {
    value.trim().parse::<IpAddr>().ok().map(IpNetwork::from)
}

pub struct AuthSession {
    pub session: CurrentSession,
    pub jar: CookieJar,
}

impl FromRequestParts<AppState> for AuthSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let Some(cookie) = jar.get(session_cookie_name(&state.config)) else {
            return Err(AppError::Unauthenticated);
        };
        let Some(session) = auth::load(state, cookie.value()).await? else {
            return Err(AppError::Unauthenticated);
        };
        Ok(AuthSession { session, jar })
    }
}

pub struct Csrf;

impl FromRequestParts<AppState> for Csrf {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        if matches!(parts.method.as_str(), "GET" | "HEAD" | "OPTIONS") {
            return Ok(Csrf);
        }
        let jar = CookieJar::from_headers(&parts.headers);
        let cookie = jar
            .get(csrf_cookie_name(&state.config))
            .map(|cookie| cookie.value())
            .unwrap_or("");
        let header = parts
            .headers
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");
        if cookie.len() < 20 || !ct_eq_str(cookie, header) {
            return Err(AppError::Forbidden("Refresh the page and try again."));
        }
        if let Some(origin) = parts
            .headers
            .get("origin")
            .and_then(|value| value.to_str().ok())
        {
            if origin.trim_end_matches('/') != state.config.app_base_url {
                return Err(AppError::Forbidden("This origin is not allowed."));
            }
        } else if parts
            .headers
            .get("sec-fetch-site")
            .and_then(|value| value.to_str().ok())
            == Some("cross-site")
        {
            return Err(AppError::Forbidden("This origin is not allowed."));
        }
        Ok(Csrf)
    }
}

pub struct OptionalSession(pub Option<AuthSession>);

impl FromRequestParts<AppState> for OptionalSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        match AuthSession::from_request_parts(parts, state).await {
            Ok(session) => Ok(OptionalSession(Some(session))),
            Err(AppError::Unauthenticated) => Ok(OptionalSession(None)),
            Err(err) => Err(err),
        }
    }
}

pub struct AdminSession(pub AuthSession);

impl FromRequestParts<AppState> for AdminSession {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let auth = AuthSession::from_request_parts(parts, state).await?;
        if !auth.session.is_admin {
            return Err(AppError::Forbidden("Admin access is required."));
        }
        let methods = auth::enabled_mfa_methods(state, auth.session.user_id).await?;
        if !methods.iter().any(|method| method == "totp") {
            return Err(AppError::Forbidden(
                "Admin access requires an authenticator app.",
            ));
        }
        Ok(AdminSession(auth))
    }
}

pub fn require_recent_auth(state: &AppState, session: &CurrentSession) -> Result<(), AppError> {
    auth::require_step_up(state, session)
}
