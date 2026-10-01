use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use knotree_accounts::{connect, for_tests, router};
use reqwest::header::LOCATION;
use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::net::SocketAddr;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::Mutex;
use totp_rs::{Algorithm, TOTP};

static LOCK: OnceLock<Mutex<()>> = OnceLock::new();

struct Api {
    base: String,
    http: Client,
}

#[tokio::test]
async fn identity_platform_flows() {
    let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().await;
    let db_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://knotree:knotree@127.0.0.1/knotree_accounts_test".into());
    let config = for_tests(&db_url).expect("test config");
    let state = connect(config).await.expect("database");
    sqlx::query(
        "TRUNCATE users, oauth_clients, auth_attempts, email_messages, security_events, social_transactions RESTART IDENTITY CASCADE",
    )
    .execute(&state.db)
    .await
    .unwrap();
    sqlx::query(
        r#"
        INSERT INTO oauth_clients (
            id, name, client_type, redirect_uris, allowed_scopes, first_party, require_pkce, status, created_at
        ) VALUES (
            'knotree-study', 'Knotree Study', 'public',
            ARRAY['https://study.knotree.com/auth/callback'],
            ARRAY['openid','profile','email','offline_access','study:read','study:write'],
            TRUE, TRUE, 'active', now()
        )
        "#,
    )
    .execute(&state.db)
    .await
    .unwrap();

    let app = router(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr: SocketAddr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let api = Api {
        base: format!("http://{addr}"),
        http: Client::builder()
            .cookie_store(true)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap(),
    };

    let email = format!("ada-{}@example.com", uuid_suffix());
    let password = "correct horse battery";
    api.csrf().await;
    let created = api
        .post(
            "/api/v1/auth/register",
            json!({"email": email, "password": password, "password_confirm": password}),
        )
        .await;
    assert_eq!(created.status(), StatusCode::OK, "{}", created.text);

    let duplicate = api
        .post(
            "/api/v1/auth/register",
            json!({"email": email, "password": password, "password_confirm": password}),
        )
        .await;
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);

    let token = api.mailbox_token(&email, "verify-email").await;
    let verified = api
        .post("/api/v1/auth/email/verify", json!({"token": token}))
        .await;
    assert_eq!(verified.status(), StatusCode::OK, "{}", verified.text);

    let bad_login = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": "not-the-password"}),
        )
        .await;
    assert_eq!(bad_login.status(), StatusCode::UNAUTHORIZED);
    assert!(bad_login.text.contains("email or password"));
    assert!(!bad_login.text.to_lowercase().contains("does not exist"));

    let missing_csrf = api
        .http
        .post(format!("{}{}", api.base, "/api/v1/auth/login"))
        .json(&json!({"email": email, "password": password}))
        .send()
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::FORBIDDEN);

    let evil = api
        .http
        .post(format!("{}/api/v1/auth/login", api.base))
        .header("origin", "https://evil.example")
        .header("x-csrf-token", api.csrf_token().await)
        .json(&json!({"email": email, "password": password}))
        .send()
        .await
        .unwrap();
    assert_eq!(evil.status(), StatusCode::FORBIDDEN);

    let login = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": password}),
        )
        .await;
    assert_eq!(login.json["status"], "authenticated", "{}", login.text);
    let me = api.get("/api/v1/me").await;
    assert_eq!(me.json["email"], email);
    assert_eq!(me.json["email_verified"], true);
    assert_eq!(me.json["is_admin"], false);
    sqlx::query(
        "UPDATE sessions SET last_active_at = now() - interval '30 minutes', expires_at = now() + interval '2 hours' WHERE revoked_at IS NULL",
    )
    .execute(&state.db)
    .await
    .unwrap();
    let resumed = api.get_response("/api/v1/me").await;
    assert_eq!(resumed.status(), StatusCode::OK);
    let refreshed = resumed
        .headers()
        .get_all(reqwest::header::SET_COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .any(|value| {
            let lower = value.to_ascii_lowercase();
            lower.contains("knotree_session=")
                && lower.contains("max-age=")
                && lower.contains("expires=")
        });
    assert!(
        refreshed,
        "activity should refresh the persistent session cookie"
    );
    let extended: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sessions WHERE revoked_at IS NULL AND expires_at > now() + interval '10 days')",
    )
    .fetch_one(&state.db)
    .await
    .unwrap();
    assert!(extended, "activity should slide the server session expiry");

    let denied = api.get("/api/v1/admin/stats").await;
    assert_eq!(denied.status, StatusCode::FORBIDDEN);
    let escalate = api
        .post_method_patch(
            "/api/v1/me/profile",
            json!({"display_name": "Ada Lovelace", "is_admin": true}),
        )
        .await;
    assert_eq!(escalate.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let renamed = api
        .post_method_patch(
            "/api/v1/me/profile",
            json!({"display_name": "Ada Lovelace"}),
        )
        .await;
    assert_eq!(renamed.status(), StatusCode::OK, "{}", renamed.text);

    let setup = api.post("/api/v1/me/mfa/totp/setup", json!({})).await;
    assert_eq!(setup.status(), StatusCode::OK, "{}", setup.text);
    let secret = setup.json["secret"].as_str().unwrap().replace(' ', "");
    let code = totp_code(&secret, &email, 0);
    let confirmed = api
        .post("/api/v1/me/mfa/totp/confirm", json!({"code": code}))
        .await;
    assert_eq!(confirmed.status(), StatusCode::OK, "{}", confirmed.text);
    let recovery = confirmed.json["recovery_codes"].as_array().unwrap();
    assert_eq!(recovery.len(), 8);

    api.post("/api/v1/auth/logout", json!({})).await;
    let mfa_login = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": password}),
        )
        .await;
    assert_eq!(mfa_login.json["status"], "mfa_required");
    let mfa_token = mfa_login.json["mfa_token"].as_str().unwrap().to_string();
    let bad_code = api
        .post(
            "/api/v1/auth/mfa/verify",
            json!({"mfa_token": mfa_token, "method": "totp", "code": "000000"}),
        )
        .await;
    assert_eq!(bad_code.status(), StatusCode::BAD_REQUEST);
    let good = api
        .post(
            "/api/v1/auth/mfa/verify",
            json!({"mfa_token": mfa_token, "method": "totp", "code": totp_code(&secret, &email, 1)}),
        )
        .await;
    assert_eq!(good.json["status"], "authenticated", "{}", good.text);

    api.post("/api/v1/auth/logout", json!({})).await;
    let recovery_login = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": password}),
        )
        .await;
    let recovery_token = recovery_login.json["mfa_token"]
        .as_str()
        .unwrap()
        .to_string();
    let recovery_code = recovery[0].as_str().unwrap();
    let recovered = api
        .post(
            "/api/v1/auth/mfa/verify",
            json!({"mfa_token": recovery_token, "method": "recovery", "code": recovery_code}),
        )
        .await;
    assert_eq!(recovered.status(), StatusCode::OK, "{}", recovered.text);
    api.post("/api/v1/auth/logout", json!({})).await;
    let reuse_login = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": password}),
        )
        .await;
    let reuse = api
        .post(
            "/api/v1/auth/mfa/verify",
            json!({"mfa_token": reuse_login.json["mfa_token"], "method": "recovery", "code": recovery_code}),
        )
        .await;
    assert_ne!(reuse.status(), StatusCode::OK);
    let continued = api
        .post(
            "/api/v1/auth/mfa/verify",
            json!({"mfa_token": reuse_login.json["mfa_token"], "method": "recovery", "code": recovery[1].as_str().unwrap()}),
        )
        .await;
    assert_eq!(continued.status(), StatusCode::OK, "{}", continued.text);

    let sessions = api.get("/api/v1/me/sessions").await;
    assert!(sessions.json["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["current"] == true));

    let verifier =
        URL_SAFE_NO_PAD.encode(Sha256::digest(b"verifier-material-for-knotree-study-pkce"));
    // 32 bytes of a fixed string may be short; build a long verifier.
    let verifier = format!("{verifier}{verifier}");
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()));
    let bad_redirect = api
        .get_status(&format!(
            "/oauth/authorize?client_id=knotree-study&redirect_uri={}&response_type=code&scope=openid&state=state-value-1&code_challenge={challenge}&code_challenge_method=S256",
            url_encode("https://evil.example/callback")
        ))
        .await;
    assert_eq!(bad_redirect, StatusCode::BAD_REQUEST);

    let authorize = api
        .get_response(&format!(
            "/oauth/authorize?client_id=knotree-study&redirect_uri={}&response_type=code&scope={}&state=state-value-1&code_challenge={challenge}&code_challenge_method=S256&nonce=nonce-value-1",
            url_encode("https://study.knotree.com/auth/callback"),
            url_encode("openid profile email offline_access")
        ))
        .await;
    assert!(
        authorize.status().is_redirection(),
        "{}",
        authorize.status()
    );
    let location = authorize
        .headers()
        .get(LOCATION)
        .unwrap()
        .to_str()
        .unwrap()
        .to_string();
    let code = query_param(&location, "code").expect("authorization code");
    assert_eq!(
        query_param(&location, "state").as_deref(),
        Some("state-value-1")
    );

    let token = api
        .form(
            "/oauth/token",
            [
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", "https://study.knotree.com/auth/callback"),
                ("client_id", "knotree-study"),
                ("code_verifier", verifier.as_str()),
            ],
        )
        .await;
    assert_eq!(token.status(), StatusCode::OK, "{}", token.text);
    assert!(token.json["id_token"].as_str().unwrap().split('.').count() == 3);
    assert!(token.json["refresh_token"].is_string());
    let access = token.json["access_token"].as_str().unwrap().to_string();
    let refresh = token.json["refresh_token"].as_str().unwrap().to_string();
    let userinfo = api
        .http
        .get(format!("{}/oauth/userinfo", api.base))
        .bearer_auth(&access)
        .send()
        .await
        .unwrap();
    assert_eq!(userinfo.status(), StatusCode::OK);
    let userinfo: Value = userinfo.json().await.unwrap();
    assert_eq!(userinfo["email"], email);

    let reused_code = api
        .form(
            "/oauth/token",
            [
                ("grant_type", "authorization_code"),
                ("code", code.as_str()),
                ("redirect_uri", "https://study.knotree.com/auth/callback"),
                ("client_id", "knotree-study"),
                ("code_verifier", verifier.as_str()),
            ],
        )
        .await;
    assert_eq!(reused_code.status(), StatusCode::BAD_REQUEST);

    let discovery = api.get("/.well-known/openid-configuration").await;
    assert!(discovery.json["jwks_uri"]
        .as_str()
        .unwrap()
        .ends_with("/.well-known/jwks.json"));
    let jwks = api.get("/.well-known/jwks.json").await;
    assert!(!jwks.json["keys"].as_array().unwrap().is_empty());

    let forgot = api
        .post(
            "/api/v1/auth/password/forgot",
            json!({"email": "nobody@example.com"}),
        )
        .await;
    assert_eq!(forgot.status(), StatusCode::OK);
    assert!(forgot.text.contains("If an account exists"));
    api.post("/api/v1/auth/password/forgot", json!({"email": email}))
        .await;
    let reset_token = api.mailbox_token(&email, "reset-password").await;
    let reset = api
        .post(
            "/api/v1/auth/password/reset",
            json!({"token": reset_token, "password": "a different passphrase"}),
        )
        .await;
    assert_eq!(reset.status(), StatusCode::OK, "{}", reset.text);
    let old = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": password}),
        )
        .await;
    assert_eq!(old.status(), StatusCode::UNAUTHORIZED);
    let fresh = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": "a different passphrase"}),
        )
        .await;
    assert_eq!(fresh.json["status"], "mfa_required");
    api.post(
        "/api/v1/auth/mfa/verify",
        json!({"mfa_token": fresh.json["mfa_token"], "method": "recovery", "code": recovery[2].as_str().unwrap()}),
    )
    .await;

    sqlx::query("INSERT INTO role_assignments (user_id, role, created_at) VALUES ($1, 'super_admin', now())")
        .bind(me.json["id"].as_str().unwrap().parse::<uuid::Uuid>().unwrap())
        .execute(&state.db)
        .await
        .unwrap();
    // Session cached admin flag at login time. Sign in again.
    api.post("/api/v1/auth/logout", json!({})).await;
    let admin_login = api
        .post(
            "/api/v1/auth/login",
            json!({"email": email, "password": "a different passphrase"}),
        )
        .await;
    let admin_verified = api
        .post(
            "/api/v1/auth/mfa/verify",
            json!({"mfa_token": admin_login.json["mfa_token"], "method": "recovery", "code": recovery[3].as_str().unwrap()}),
        )
        .await;
    assert_eq!(
        admin_verified.status(),
        StatusCode::OK,
        "{}",
        admin_verified.text
    );
    let stats = api.get("/api/v1/admin/stats").await;
    assert_eq!(stats.status, StatusCode::OK, "{}", stats.text);
    assert!(stats.json["users"]["total"].as_i64().unwrap() >= 1);
    let users = api.get("/api/v1/admin/users?q=ada").await;
    assert_eq!(users.status, StatusCode::OK);
    let logs = api.get("/api/v1/admin/security-events").await;
    assert_eq!(logs.status, StatusCode::OK);
    assert!(logs.json["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(
            |item| item["event_type"] == "USER_REGISTERED" || item["event_type"] == "LOGIN_SUCCESS"
        ));

    let _ = refresh;
}

#[tokio::test]
async fn configured_super_admin_must_be_verified_before_existing_admin_is_replaced() {
    let _guard = LOCK.get_or_init(|| Mutex::new(())).lock().await;
    let db_url = std::env::var("TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://knotree:knotree@127.0.0.1/knotree_accounts_test".into());
    let verified_id = uuid::Uuid::now_v7();
    let mut verified_config = for_tests(&db_url).expect("test config");
    verified_config.super_admin_user_id = Some(verified_id);
    let verified_state = connect(verified_config).await.expect("database");
    sqlx::query("TRUNCATE users RESTART IDENTITY CASCADE")
        .execute(&verified_state.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO users (id,status,created_at,updated_at,password_changed_at) VALUES ($1,'active',now(),now(),now())")
        .bind(verified_id)
        .execute(&verified_state.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_emails (id,user_id,email,is_primary,verified_at,created_at) VALUES ($1,$2,$3,TRUE,now(),now())")
        .bind(uuid::Uuid::now_v7())
        .bind(verified_id)
        .bind(format!("verified-{}@example.com", uuid_suffix()))
        .execute(&verified_state.db)
        .await
        .unwrap();
    knotree_accounts::auth::bootstrap_admin(&verified_state)
        .await
        .expect("verified active user becomes administrator");

    let unverified_id = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO users (id,status,created_at,updated_at,password_changed_at) VALUES ($1,'active',now(),now(),now())")
        .bind(unverified_id)
        .execute(&verified_state.db)
        .await
        .unwrap();
    sqlx::query("INSERT INTO user_emails (id,user_id,email,is_primary,created_at) VALUES ($1,$2,$3,TRUE,now())")
        .bind(uuid::Uuid::now_v7())
        .bind(unverified_id)
        .bind(format!("unverified-{}@example.com", uuid_suffix()))
        .execute(&verified_state.db)
        .await
        .unwrap();
    let mut unverified_config = for_tests(&db_url).expect("test config");
    unverified_config.super_admin_user_id = Some(unverified_id);
    let unverified_state = connect(unverified_config).await.expect("database");
    assert!(knotree_accounts::auth::bootstrap_admin(&unverified_state)
        .await
        .is_err());

    let admins: Vec<uuid::Uuid> =
        sqlx::query_scalar("SELECT user_id FROM role_assignments WHERE role = 'super_admin'")
            .fetch_all(&verified_state.db)
            .await
            .unwrap();
    assert_eq!(admins, vec![verified_id]);
}

struct Response {
    status: StatusCode,
    text: String,
    json: Value,
}

impl Api {
    async fn csrf(&self) -> String {
        self.csrf_token().await
    }

    async fn csrf_token(&self) -> String {
        let response = self
            .http
            .get(format!("{}/api/v1/auth/csrf", self.base))
            .send()
            .await
            .unwrap();
        let body: Value = response.json().await.unwrap();
        body["csrf_token"].as_str().unwrap().to_string()
    }

    async fn post(&self, path: &str, body: Value) -> Response {
        self.send(self.http.post(format!("{}{path}", self.base)).json(&body))
            .await
    }

    async fn post_method_patch(&self, path: &str, body: Value) -> Response {
        let token = self.csrf_token().await;
        self.send(
            self.http
                .patch(format!("{}{path}", self.base))
                .header("origin", "http://localhost:5173")
                .header("x-csrf-token", token)
                .json(&body),
        )
        .await
    }

    async fn get(&self, path: &str) -> Response {
        self.send(self.http.get(format!("{}{path}", self.base)))
            .await
    }

    async fn get_status(&self, path: &str) -> StatusCode {
        self.http
            .get(format!("{}{path}", self.base))
            .send()
            .await
            .unwrap()
            .status()
    }

    async fn get_response(&self, path: &str) -> reqwest::Response {
        self.http
            .get(format!("{}{path}", self.base))
            .send()
            .await
            .unwrap()
    }

    async fn form(&self, path: &str, form: [(&str, &str); 5]) -> Response {
        let response = self
            .http
            .post(format!("{}{path}", self.base))
            .form(&form)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        let json = serde_json::from_str(&text).unwrap_or(Value::Null);
        Response { status, text, json }
    }

    async fn send(&self, builder: reqwest::RequestBuilder) -> Response {
        let token = self.csrf_token().await;
        let response = builder
            .header("origin", "http://localhost:5173")
            .header("x-csrf-token", token)
            .send()
            .await
            .unwrap();
        let status = response.status();
        let text = response.text().await.unwrap_or_default();
        let json = serde_json::from_str(&text).unwrap_or(Value::Null);
        Response { status, text, json }
    }

    async fn mailbox_token(&self, email: &str, template: &str) -> String {
        let response = self
            .http
            .get(format!("{}/api/v1/dev/mailbox?email={email}", self.base))
            .send()
            .await
            .unwrap();
        let body: Value = response.json().await.unwrap();
        let text = body["messages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|message| message["template"] == template)
            .unwrap_or_else(|| panic!("missing {template} email: {body}"))["text"]
            .as_str()
            .unwrap();
        text.split("token=")
            .nth(1)
            .unwrap()
            .split_whitespace()
            .next()
            .unwrap()
            .trim_matches(|c: char| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
            .to_string()
    }
}

fn totp_code(secret: &str, email: &str, step_offset: i64) -> String {
    let totp = TOTP::new(
        Algorithm::SHA1,
        6,
        1,
        30,
        totp_rs::Secret::Encoded(secret.to_string())
            .to_bytes()
            .unwrap(),
        Some("Knotree".into()),
        email.into(),
    )
    .unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs() as i64;
    let step = now / 30 + step_offset;
    totp.generate((step as u64) * 30)
}

fn uuid_suffix() -> String {
    uuid::Uuid::now_v7().simple().to_string()
}

fn url_encode(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

fn query_param(url: &str, key: &str) -> Option<String> {
    url.split('?').nth(1)?.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        if name == key {
            Some(urlencoding_decode(value))
        } else {
            None
        }
    })
}

fn urlencoding_decode(value: &str) -> String {
    url::form_urlencoded::parse(format!("k={value}").as_bytes())
        .next()
        .map(|(_, decoded)| decoded.into_owned())
        .unwrap_or_else(|| value.to_string())
}

impl Response {
    fn status(&self) -> StatusCode {
        self.status
    }
}
