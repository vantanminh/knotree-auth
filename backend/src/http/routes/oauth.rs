use crate::auth::{self, ClientMeta};
use crate::error::AppError;
use crate::http::extract::{AuthSession, Csrf, Meta};
use crate::oauth::{self, AuthorizeInput, CodeExchange, OAuthFailure};
use crate::state::AppState;
use axum::extract::{Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::{Form, Json};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Deserialize)]
pub struct AuthorizeQuery {
    client_id: String,
    redirect_uri: String,
    response_type: String,
    scope: String,
    state: String,
    code_challenge: String,
    code_challenge_method: String,
    nonce: Option<String>,
}

pub async fn authorize(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AuthorizeQuery>,
    Meta(meta): Meta,
) -> Response {
    let input = AuthorizeInput {
        client_id: query.client_id.clone(),
        redirect_uri: query.redirect_uri.clone(),
        response_type: query.response_type.clone(),
        scope: query.scope.clone(),
        state: query.state.clone(),
        code_challenge: query.code_challenge.clone(),
        code_challenge_method: query.code_challenge_method.clone(),
        nonce: query.nonce.clone(),
    };
    let valid = match oauth::validate_authorize(&state, input).await {
        Ok(valid) => valid,
        Err(err) => {
            if query_redirect_is_safe(&state, &query.client_id, &query.redirect_uri).await
                && !matches!(err, OAuthFailure::InvalidClient)
            {
                return redirect_error(&query.redirect_uri, &err, &query.state);
            }
            return oauth_response(StatusCode::BAD_REQUEST, &err);
        }
    };
    let jar = axum_extra::extract::cookie::CookieJar::from_headers(&headers);
    let token = jar
        .get(crate::http::extract::session_cookie_name(&state.config))
        .map(|cookie| cookie.value().to_string());
    let loaded = if let Some(token) = token.as_deref() {
        auth::load(&state, token).await.ok().flatten()
    } else {
        None
    };
    let Some(loaded) = loaded else {
        let target = format!(
            "/sign-in?return_to={}",
            urlencoding_query(&format!(
                "/oauth/authorize?{}",
                build_authorize_query(&query)
            ))
        );
        return Redirect::temporary(&target).into_response();
    };
    if loaded.refresh_cookie {
        if let Some(token) = token.as_deref() {
            let max_age = (loaded.session.expires_at - chrono::Utc::now())
                .num_seconds()
                .max(60);
            crate::http::extract::note_cookie_renewal(
                crate::http::extract::session_cookie_name(&state.config),
                token,
                max_age,
            );
        }
    }
    let session = loaded.session;
    if !valid.client.first_party {
        let id = uuid::Uuid::now_v7();
        let now = chrono::Utc::now();
        if sqlx::query(
            r#"
            INSERT INTO oauth_consent_requests (
                id, client_id, redirect_uri, scopes, state, nonce, code_challenge, code_challenge_method, expires_at, created_at
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,'S256',$8,$9)
            "#,
        )
        .bind(id)
        .bind(&valid.client.id)
        .bind(&valid.redirect_uri)
        .bind(&valid.scopes)
        .bind(&valid.state)
        .bind(&valid.nonce)
        .bind(&valid.code_challenge)
        .bind(now + chrono::Duration::minutes(10))
        .bind(now)
        .execute(&state.db)
        .await
        .is_err()
        {
            return oauth_response(StatusCode::INTERNAL_SERVER_ERROR, &OAuthFailure::InvalidRequest("Could not start consent."));
        }
        return Redirect::temporary(&format!("/oauth/consent?request={id}")).into_response();
    }
    match oauth::issue_code(&state, &valid, session.user_id, session.id, &meta).await {
        Ok(code) => redirect_code(&valid.redirect_uri, &code, &valid.state),
        Err(_) => oauth_response(
            StatusCode::INTERNAL_SERVER_ERROR,
            &OAuthFailure::InvalidRequest("Could not authorize."),
        ),
    }
}

#[derive(Deserialize)]
pub struct ConsentBody {
    request_id: uuid::Uuid,
    approve: bool,
}

pub async fn consent(
    State(state): State<AppState>,
    auth: AuthSession,
    Meta(meta): Meta,
    Csrf: Csrf,
    Json(body): Json<ConsentBody>,
) -> Result<Json<Value>, AppError> {
    if !body.approve {
        return Err(AppError::Forbidden("The request was denied."));
    }
    let row: Option<(String, String, Vec<String>, String, Option<String>, String)> =
        sqlx::query_as(
            r#"
        SELECT client_id, redirect_uri, scopes, state, nonce, code_challenge
        FROM oauth_consent_requests
        WHERE id = $1 AND expires_at > now()
        "#,
        )
        .bind(body.request_id)
        .fetch_optional(&state.db)
        .await?;
    let Some((client_id, redirect_uri, scopes, state_value, nonce, code_challenge)) = row else {
        return Err(AppError::Gone("This authorization request expired."));
    };
    let valid = oauth::validate_authorize(
        &state,
        AuthorizeInput {
            client_id,
            redirect_uri,
            response_type: "code".into(),
            scope: scopes.join(" "),
            state: state_value,
            code_challenge,
            code_challenge_method: "S256".into(),
            nonce,
        },
    )
    .await
    .map_err(|_| AppError::Validation("This authorization request is no longer valid."))?;
    let code =
        oauth::issue_code(&state, &valid, auth.session.user_id, auth.session.id, &meta).await?;
    sqlx::query("DELETE FROM oauth_consent_requests WHERE id = $1")
        .bind(body.request_id)
        .execute(&state.db)
        .await?;
    let joiner = if valid.redirect_uri.contains('?') {
        '&'
    } else {
        '?'
    };
    Ok(Json(json!({
        "redirect_to": format!(
            "{}{joiner}code={}&state={}",
            valid.redirect_uri,
            urlencoding_query(&code),
            urlencoding_query(&valid.state)
        )
    })))
}

#[derive(Deserialize)]
pub struct ContextQuery {
    return_to: String,
}

pub async fn context(
    State(state): State<AppState>,
    Query(query): Query<ContextQuery>,
) -> Json<Value> {
    let Some(safe) = crate::security::redirect::safe_return_to(&query.return_to) else {
        return Json(json!({"client_name": null}));
    };
    let Ok(url) = url::Url::parse(&format!("http://accounts.local{safe}")) else {
        return Json(json!({"client_name": null}));
    };
    let Some(client_id) = url
        .query_pairs()
        .find(|(key, _)| key == "client_id")
        .map(|(_, value)| value.into_owned())
    else {
        return Json(json!({"client_name": null}));
    };
    match oauth::load_client(&state, &client_id).await {
        Ok(Some(client)) if client.status == "active" => {
            Json(json!({"client_name": client.name, "client_id": client.id}))
        }
        _ => Json(json!({"client_name": null})),
    }
}

#[derive(Deserialize)]
pub struct TokenForm {
    grant_type: String,
    code: Option<String>,
    redirect_uri: Option<String>,
    client_id: Option<String>,
    client_secret: Option<String>,
    code_verifier: Option<String>,
    refresh_token: Option<String>,
    scope: Option<String>,
}

pub async fn token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Response {
    let (client_id, client_secret) = client_auth(&headers, form.client_id, form.client_secret);
    let Some(client_id) = client_id else {
        return oauth_response(StatusCode::UNAUTHORIZED, &OAuthFailure::InvalidClient);
    };
    let result = match form.grant_type.as_str() {
        "authorization_code" => {
            oauth::exchange_code(
                &state,
                CodeExchange {
                    code: form.code.unwrap_or_default(),
                    redirect_uri: form.redirect_uri.unwrap_or_default(),
                    client_id,
                    client_secret,
                    code_verifier: form.code_verifier.unwrap_or_default(),
                },
            )
            .await
        }
        "refresh_token" => {
            oauth::refresh(
                &state,
                &client_id,
                client_secret.as_deref(),
                &form.refresh_token.unwrap_or_default(),
            )
            .await
        }
        "client_credentials" => {
            oauth::client_credentials(
                &state,
                &client_id,
                client_secret.as_deref(),
                form.scope.as_deref().unwrap_or(""),
            )
            .await
        }
        _ => Err(OAuthFailure::UnsupportedGrant),
    };
    match result {
        Ok(token) => (StatusCode::OK, Json(token)).into_response(),
        Err(err) => oauth_response(
            if matches!(err, OAuthFailure::InvalidClient) {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::BAD_REQUEST
            },
            &err,
        ),
    }
}

pub async fn revoke(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<TokenForm>,
) -> Response {
    let (client_id, client_secret) = client_auth(&headers, form.client_id, form.client_secret);
    let Some(client_id) = client_id else {
        return oauth_response(StatusCode::UNAUTHORIZED, &OAuthFailure::InvalidClient);
    };
    let token = form.code.or(form.refresh_token).unwrap_or_default();
    match oauth::revoke(&state, &client_id, client_secret.as_deref(), &token).await {
        Ok(()) => StatusCode::OK.into_response(),
        Err(err) => oauth_response(StatusCode::UNAUTHORIZED, &err),
    }
}

#[derive(Deserialize)]
pub struct IntrospectForm {
    token: String,
    client_id: Option<String>,
    client_secret: Option<String>,
}

pub async fn introspect(
    State(state): State<AppState>,
    headers: HeaderMap,
    Form(form): Form<IntrospectForm>,
) -> Response {
    let (client_id, client_secret) = client_auth(&headers, form.client_id, form.client_secret);
    let Some(client_id) = client_id else {
        return oauth_response(StatusCode::UNAUTHORIZED, &OAuthFailure::InvalidClient);
    };
    match oauth::introspect(&state, &client_id, client_secret.as_deref(), &form.token).await {
        Ok(body) => (StatusCode::OK, Json(body)).into_response(),
        Err(err) => oauth_response(StatusCode::UNAUTHORIZED, &err),
    }
}

pub async fn userinfo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, AppError> {
    let token = bearer(
        headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok()),
    )
    .ok_or(AppError::Unauthenticated)?;
    Ok(Json(oauth::userinfo(&state, &token).await?))
}

pub async fn discovery(State(state): State<AppState>) -> Json<Value> {
    Json(oauth::discovery(&state.config))
}

pub async fn jwks(State(state): State<AppState>) -> Json<Value> {
    Json(oauth::jwks(&state.config))
}

fn client_auth(
    headers: &HeaderMap,
    id: Option<String>,
    secret: Option<String>,
) -> (Option<String>, Option<String>) {
    if let Some((id, secret)) = oauth::basic_client(
        headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok()),
    ) {
        return (Some(id), Some(secret));
    }
    (id, secret)
}

fn bearer(value: Option<&str>) -> Option<String> {
    value?
        .strip_prefix("Bearer ")
        .map(|token| token.trim().to_string())
        .filter(|token| !token.is_empty())
}

fn oauth_response(status: StatusCode, err: &OAuthFailure) -> Response {
    (
        status,
        Json(json!({"error": err.code(), "error_description": err.description()})),
    )
        .into_response()
}

fn redirect_error(redirect_uri: &str, err: &OAuthFailure, state: &str) -> Response {
    let location = format!(
        "{redirect_uri}?error={}&error_description={}&state={}",
        urlencoding_query(err.code()),
        urlencoding_query(err.description()),
        urlencoding_query(state)
    );
    Redirect::temporary(&location).into_response()
}

fn redirect_code(redirect_uri: &str, code: &str, state: &str) -> Response {
    let joiner = if redirect_uri.contains('?') { '&' } else { '?' };
    Redirect::temporary(&format!(
        "{redirect_uri}{joiner}code={}&state={}",
        urlencoding_query(code),
        urlencoding_query(state)
    ))
    .into_response()
}

fn urlencoding_query(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

async fn query_redirect_is_safe(state: &AppState, client_id: &str, redirect_uri: &str) -> bool {
    match oauth::load_client(state, client_id).await {
        Ok(Some(client)) => {
            crate::security::redirect::exact_redirect_allowed(&client.redirect_uris, redirect_uri)
        }
        _ => false,
    }
}

fn build_authorize_query(query: &AuthorizeQuery) -> String {
    let mut serializer = url::form_urlencoded::Serializer::new(String::new());
    serializer.append_pair("client_id", &query.client_id);
    serializer.append_pair("redirect_uri", &query.redirect_uri);
    serializer.append_pair("response_type", &query.response_type);
    serializer.append_pair("scope", &query.scope);
    serializer.append_pair("state", &query.state);
    serializer.append_pair("code_challenge", &query.code_challenge);
    serializer.append_pair("code_challenge_method", &query.code_challenge_method);
    if let Some(nonce) = &query.nonce {
        serializer.append_pair("nonce", nonce);
    }
    serializer.finish()
}

#[allow(dead_code)]
fn _meta(meta: ClientMeta) -> ClientMeta {
    meta
}
