pub(crate) mod extract;
mod routes;

use crate::error::REQUEST_ID;
use crate::i18n::{Locale, REQUEST_LOCALE};
use crate::state::AppState;
use axum::extract::Request;
use axum::http::{header, HeaderName, HeaderValue, Method};
use axum::middleware::Next;
use axum::response::Response;
use axum::Router;
use std::time::Instant;
use tower_http::cors::{AllowOrigin, CorsLayer};
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::timeout::TimeoutLayer;
use uuid::Uuid;

pub fn router(state: AppState) -> Router {
    let cors = cors_layer(&state);
    let small_bodies = Router::new()
        .merge(routes::public_routes())
        .merge(routes::api_routes())
        .layer(RequestBodyLimitLayer::new(64 * 1024));
    let uploads = routes::upload_routes().layer(RequestBodyLimitLayer::new(
        crate::admin::clients::LOGO_MAX_BYTES + 64 * 1024,
    ));
    Router::new()
        .merge(small_bodies)
        .merge(uploads)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            security_headers,
        ))
        .layer(axum::middleware::from_fn(request_context))
        .layer(cors)
        .layer(TimeoutLayer::with_status_code(
            axum::http::StatusCode::GATEWAY_TIMEOUT,
            std::time::Duration::from_secs(30),
        ))
        .with_state(state)
}

fn cors_layer(state: &AppState) -> CorsLayer {
    let origins: Vec<HeaderValue> = state
        .config
        .cors_origins
        .iter()
        .filter_map(|origin| origin.parse().ok())
        .collect();
    CorsLayer::new()
        .allow_origin(AllowOrigin::list(origins))
        .allow_credentials(true)
        .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
        .allow_headers([
            header::CONTENT_TYPE,
            header::AUTHORIZATION,
            HeaderName::from_static("x-csrf-token"),
        ])
}

async fn request_context(req: Request, next: Next) -> Response {
    let request_id = format!("req_{}", Uuid::now_v7().simple());
    let request_id_for_log = request_id.clone();
    let started = Instant::now();
    let method = req.method().clone();
    let path = req.uri().path().to_string();
    let locale = Locale::from_accept_language(
        req.headers()
            .get(header::ACCEPT_LANGUAGE)
            .and_then(|value| value.to_str().ok()),
    );
    let response = REQUEST_ID
        .scope(
            request_id.clone(),
            REQUEST_LOCALE.scope(locale, async move {
                extract::scope_cookie_renewal(async move {
                    let mut response = next.run(req).await;
                    if let Ok(value) = HeaderValue::from_str(&request_id) {
                        response.headers_mut().insert("x-request-id", value);
                    }
                    response
                })
                .await
            }),
        )
        .await;
    let status = response.status().as_u16().to_string();
    metrics::counter!("http_requests_total", "method" => method.to_string(), "status" => status)
        .increment(1);
    metrics::histogram!("http_request_duration_seconds").record(started.elapsed().as_secs_f64());
    if response.status().is_server_error() {
        tracing::error!(request_id = %request_id_for_log, %method, %path, status = %response.status(), "request failed");
    } else {
        tracing::info!(request_id = %request_id_for_log, %method, %path, status = %response.status(), "request");
    }
    response
}

async fn security_headers(
    state: axum::extract::State<AppState>,
    req: Request,
    next: Next,
) -> Response {
    let mut response = next.run(req).await;
    let headers = response.headers_mut();
    insert(headers, "x-content-type-options", "nosniff");
    insert(headers, "x-frame-options", "DENY");
    insert(headers, "referrer-policy", "no-referrer");
    insert(
        headers,
        "permissions-policy",
        "camera=(), microphone=(), geolocation=()",
    );
    insert(
        headers,
        "content-security-policy",
        "default-src 'none'; frame-ancestors 'none'; base-uri 'none'",
    );
    insert(headers, "cache-control", "no-store");
    if let Some(renewal) = extract::take_cookie_renewal() {
        if !extract::response_sets_cookie(headers, renewal.name) {
            let cookie = extract::build_cookie(
                &state.config,
                renewal.name,
                &renewal.value,
                true,
                renewal.max_age_secs,
            );
            if let Ok(value) = HeaderValue::from_str(&cookie.to_string()) {
                headers.append(header::SET_COOKIE, value);
            }
        }
    }
    if state.config.cookie_secure {
        insert(
            headers,
            "strict-transport-security",
            "max-age=31536000; includeSubDomains",
        );
    }
    response
}

fn insert(headers: &mut axum::http::HeaderMap, name: &'static str, value: &'static str) {
    if let Ok(name) = HeaderName::from_bytes(name.as_bytes()) {
        headers.insert(name, HeaderValue::from_static(value));
    }
}
