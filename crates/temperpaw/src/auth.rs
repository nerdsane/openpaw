//! Authentication routes and middleware for the embedded dashboard.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use argon2::Argon2;
use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use axum::body::Body;
use axum::extract::{ConnectInfo, State};
use axum::http::header::COOKIE;
use axum::http::{HeaderMap, HeaderValue, Request, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};
use temper_platform::bearer_auth::PreAuthenticatedRequest;
#[cfg(test)]
use temper_store_turso::TursoEventStore;

use crate::storage::PawStorage;

type SecretsVault = temper_server::secrets::vault::SecretsVault;

const ACCOUNTS_SECRET_KEY: &str = "auth_accounts";
const SESSION_COOKIE: &str = "paw_session";
const SESSION_DURATION_SECS: usize = 60 * 60 * 24 * 7;

#[derive(Clone)]
pub struct AuthState {
    storage: PawStorage,
    vault: Arc<SecretsVault>,
    jwt_secret: Arc<Vec<u8>>,
    tenant: Arc<String>,
    cookie_secure: bool,
    register_lock: Arc<tokio::sync::Mutex<()>>,
}

impl AuthState {
    pub fn new(
        storage: impl Into<PawStorage>,
        vault: Arc<SecretsVault>,
        jwt_secret: Vec<u8>,
        tenant: String,
        cookie_secure: bool,
    ) -> Self {
        Self {
            storage: storage.into(),
            vault,
            jwt_secret: Arc::new(jwt_secret),
            tenant: Arc::new(tenant),
            cookie_secure,
            register_lock: Arc::new(tokio::sync::Mutex::new(())),
        }
    }

    fn tenant(&self) -> &str {
        self.tenant.as_str()
    }

    #[cfg(test)]
    async fn for_tests(root: &std::path::Path) -> Self {
        let db_path = root.join("auth-test.db");
        let store = TursoEventStore::new(&format!("file:{}", db_path.display()), None)
            .await
            .unwrap();
        let vault = Arc::new(SecretsVault::new(&[7; 32]));
        Self::new(store, vault, vec![3; 32], "default".to_string(), false)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LocalAccount {
    email: String,
    password_hash: String,
    created_at: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SessionClaims {
    email: String,
    provider: String,
    exp: usize,
}

#[derive(Debug, Deserialize)]
struct Credentials {
    email: String,
    password: String,
}

#[derive(Debug, Deserialize)]
struct ChangePasswordRequest {
    current_password: String,
    new_password: String,
}

#[derive(Debug, Serialize)]
struct ProvidersResponse {
    providers: Vec<&'static str>,
    registration_open: bool,
}

#[derive(Debug, Serialize)]
struct SessionUserResponse {
    email: String,
    provider: &'static str,
}

pub fn router(state: AuthState) -> Router {
    Router::new()
        .route("/auth/register", post(register))
        .route("/auth/login", post(login))
        .route("/auth/me", get(me))
        .route("/auth/logout", post(logout))
        .route("/auth/providers", get(providers))
        .route("/auth/change-password", post(change_password))
        .with_state(state)
}

/// Prefix covering the full family of headers the kernel's Cedar principal
/// builder (`SecurityContext::from_headers`) trusts to establish identity,
/// delegation, scope, and context: principal id/kind, `agent-role`,
/// `agent-type`, `acting-for` (delegation), `principal-scopes`, `action-context`,
/// and arbitrary `x-temper-attr-*` / `x-temper-ctx-*` / `x-temper-span-attr-*`
/// attributes. A remote client must never be able to set any of these; they are
/// honored only when injected server-side (cookie/bearer resolution) or
/// presented by a genuinely internal loopback caller. TemperPaw is the sole
/// network edge in front of the kernel, which does not itself strip these, so
/// the whole prefix is removed at the ingress edge for every external request.
const CLIENT_ASSERTABLE_IDENTITY_HEADER_PREFIX: &str = "x-temper-";

/// Additional exact header names outside the `x-temper-*` prefix that also carry
/// client-assertable identity/tenant selection.
const CLIENT_ASSERTABLE_IDENTITY_HEADERS: [&str; 2] = ["x-agent-id", "x-tenant-id"];

pub async fn middleware(
    State(state): State<AuthState>,
    mut request: Request<Body>,
    next: Next,
) -> Response {
    let path = request.uri().path().to_string();
    let method = request.method().as_str().to_string();

    let from_internal_loopback = request_is_loopback(&request);

    // Ingress edge: a request that did not originate from a trusted in-process
    // (loopback) caller must never carry client-asserted identity headers. Strip
    // them before Cedar or the kernel bearer check can trust them. Identity for
    // external requests is re-derived below only from a resolved credential
    // (session cookie or bearer token).
    if !from_internal_loopback {
        strip_client_identity_headers(request.headers_mut());
    }

    if is_safe_setup_path_public_during_bootstrap(&state, &method, &path).await
        || is_public_path(request.method().as_str(), &path)
    {
        return next.run(request).await;
    }

    if let Some(claims) = claims_from_headers(&state, request.headers()) {
        inject_auth_headers(&mut request, &claims, state.tenant());
        request.extensions_mut().insert(PreAuthenticatedRequest);
        return next.run(request).await;
    }

    if has_bearer_auth(request.headers()) {
        ensure_tenant_header(request.headers_mut(), state.tenant());
        return next.run(request).await;
    }

    // Internal WASM agent / transport calls arrive over loopback carrying
    // principal headers but no Bearer token or session cookie. Only a genuinely
    // loopback-origin request may self-assert its principal this way; external
    // callers had these headers stripped above, so they cannot reach this branch
    // with forged identity. Mark as pre-authenticated so Temper's
    // bearer_auth_check passes through.
    if from_internal_loopback
        && request.headers().contains_key("x-temper-principal-kind")
        && request.headers().contains_key("x-temper-principal-id")
    {
        ensure_tenant_header(request.headers_mut(), state.tenant());
        request.extensions_mut().insert(PreAuthenticatedRequest);
        return next.run(request).await;
    }

    if path.starts_with("/dashboard") && !is_dashboard_public_path(&path) {
        return Redirect::temporary("/dashboard/login").into_response();
    }

    StatusCode::UNAUTHORIZED.into_response()
}

/// True when the underlying TCP peer is loopback (`127.0.0.0/8` or `::1`), i.e.
/// the request came from another process inside this container rather than an
/// external client via the network edge. Derived from the real connection peer
/// address (`ConnectInfo`), never from a client-supplied forwarding header.
/// Absent connection info (e.g. in-process test transports) is treated as NOT
/// loopback — the safe default.
fn request_is_loopback(request: &Request<Body>) -> bool {
    request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(addr)| addr.ip().is_loopback())
        .unwrap_or(false)
}

/// Remove any client-asserted identity/tenant headers so downstream layers
/// (Cedar, the kernel bearer check) cannot mistake them for a server-resolved
/// principal. Strips the entire `x-temper-*` family plus the extra exact names.
fn strip_client_identity_headers(headers: &mut HeaderMap) {
    let forged: Vec<axum::http::HeaderName> = headers
        .keys()
        .filter(|name| {
            let name = name.as_str();
            name.starts_with(CLIENT_ASSERTABLE_IDENTITY_HEADER_PREFIX)
                || CLIENT_ASSERTABLE_IDENTITY_HEADERS.contains(&name)
        })
        .cloned()
        .collect();
    for name in forged {
        headers.remove(&name);
    }
}

async fn is_safe_setup_path_public_during_bootstrap(
    state: &AuthState,
    method: &str,
    path: &str,
) -> bool {
    let is_safe_bootstrap_path = matches!(
        (method, path),
        ("GET", "/paw/setup/status") | ("GET", "/paw/setup/secrets/schema")
    );

    if !is_safe_bootstrap_path {
        return false;
    }

    match load_accounts(state).await {
        Ok(accounts) => accounts.is_empty(),
        Err(error) => {
            tracing::error!(%error, path, "Could not determine bootstrap auth state");
            false
        }
    }
}

fn has_bearer_auth(headers: &HeaderMap) -> bool {
    headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .map(|value| value.starts_with("Bearer "))
        .unwrap_or(false)
}

fn is_public_path(method: &str, path: &str) -> bool {
    path.starts_with("/auth/")
        || path == "/auth"
        || path == "/healthz"
        || path == "/readyz"
        || path.starts_with("/discord/interaction")
        || path.starts_with("/triggers/webhook/")
        || (method == "GET" && (path == "/tdata" || path == "/tdata/"))
        || is_dashboard_public_path(path)
}

fn is_dashboard_public_path(path: &str) -> bool {
    if path == "/dashboard/login" || path == "/dashboard/login/" {
        return true;
    }

    path.starts_with("/dashboard/_app/")
        || path == "/dashboard/favicon.svg"
        || path == "/dashboard/robots.txt"
        || (path.starts_with("/dashboard/")
            && path
                .rsplit('/')
                .next()
                .map(|segment| segment.contains('.'))
                .unwrap_or(false))
}

fn claims_from_headers(state: &AuthState, headers: &HeaderMap) -> Option<SessionClaims> {
    let cookies = headers.get(COOKIE)?.to_str().ok()?;
    let jar = CookieJar::from_headers(headers);
    let cookie = jar.get(SESSION_COOKIE)?;
    decode::<SessionClaims>(
        cookie.value(),
        &DecodingKey::from_secret(state.jwt_secret.as_slice()),
        &Validation::new(Algorithm::HS256),
    )
    .ok()
    .map(|token| token.claims)
    .or_else(|| {
        tracing::warn!(cookies, "Invalid paw session cookie");
        None
    })
}

fn inject_auth_headers(request: &mut Request<Body>, claims: &SessionClaims, tenant: &str) {
    ensure_tenant_header(request.headers_mut(), tenant);
    request
        .headers_mut()
        .insert("x-temper-principal-kind", HeaderValue::from_static("admin"));
    if let Ok(value) = HeaderValue::from_str(&claims.email) {
        request.headers_mut().insert("x-temper-principal-id", value);
    }
}

fn ensure_tenant_header(headers: &mut HeaderMap, tenant: &str) {
    if let Ok(value) = HeaderValue::from_str(tenant) {
        headers.entry("x-tenant-id").or_insert(value);
    }
}

async fn register(
    State(state): State<AuthState>,
    jar: CookieJar,
    Json(request): Json<Credentials>,
) -> Response {
    match register_impl(&state, request).await {
        Ok(claims) => {
            let cookie = build_session_cookie(&state, &claims);
            (
                StatusCode::CREATED,
                jar.add(cookie),
                Json(SessionUserResponse {
                    email: claims.email,
                    provider: "local",
                }),
            )
                .into_response()
        }
        Err(AuthError::Validation(message)) => {
            (StatusCode::BAD_REQUEST, Json(error_body(&message))).into_response()
        }
        Err(AuthError::Conflict(message)) => {
            (StatusCode::FORBIDDEN, Json(error_body(&message))).into_response()
        }
        Err(AuthError::Storage(error)) => {
            tracing::error!(%error, "Register failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("Could not create account")),
            )
                .into_response()
        }
    }
}

async fn login(
    State(state): State<AuthState>,
    jar: CookieJar,
    Json(request): Json<Credentials>,
) -> Response {
    match login_impl(&state, request).await {
        Ok(claims) => {
            let cookie = build_session_cookie(&state, &claims);
            (
                StatusCode::OK,
                jar.add(cookie),
                Json(SessionUserResponse {
                    email: claims.email,
                    provider: "local",
                }),
            )
                .into_response()
        }
        Err(AuthError::Validation(message)) | Err(AuthError::Conflict(message)) => {
            (StatusCode::UNAUTHORIZED, Json(error_body(&message))).into_response()
        }
        Err(AuthError::Storage(error)) => {
            tracing::error!(%error, "Login failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("Could not sign in")),
            )
                .into_response()
        }
    }
}

async fn me(State(state): State<AuthState>, jar: CookieJar) -> Response {
    match current_user(&state, &jar) {
        Ok(claims) => (
            StatusCode::OK,
            Json(SessionUserResponse {
                email: claims.email,
                provider: "local",
            }),
        )
            .into_response(),
        Err(_) => StatusCode::UNAUTHORIZED.into_response(),
    }
}

async fn logout(jar: CookieJar) -> Response {
    let expired = Cookie::build((SESSION_COOKIE, ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .build();

    (StatusCode::OK, jar.remove(expired)).into_response()
}

async fn providers(State(state): State<AuthState>) -> Response {
    match load_accounts(&state).await {
        Ok(accounts) => (
            StatusCode::OK,
            Json(ProvidersResponse {
                providers: vec!["local"],
                registration_open: accounts.is_empty(),
            }),
        )
            .into_response(),
        Err(error) => {
            tracing::error!(%error, "Could not load providers");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

async fn change_password(
    State(state): State<AuthState>,
    jar: CookieJar,
    Json(request): Json<ChangePasswordRequest>,
) -> Response {
    let claims = match current_user(&state, &jar) {
        Ok(claims) => claims,
        Err(_) => return StatusCode::UNAUTHORIZED.into_response(),
    };

    if request.new_password.trim().len() < 8 {
        return (
            StatusCode::BAD_REQUEST,
            Json(error_body("New password must be at least 8 characters")),
        )
            .into_response();
    }

    match change_password_impl(&state, &claims.email, request).await {
        Ok(_) => StatusCode::OK.into_response(),
        Err(AuthError::Validation(message)) | Err(AuthError::Conflict(message)) => {
            (StatusCode::BAD_REQUEST, Json(error_body(&message))).into_response()
        }
        Err(AuthError::Storage(error)) => {
            tracing::error!(%error, "Password change failed");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(error_body("Could not update password")),
            )
                .into_response()
        }
    }
}

fn error_body(message: &str) -> serde_json::Value {
    serde_json::json!({ "error": message })
}

fn current_user(state: &AuthState, jar: &CookieJar) -> Result<SessionClaims, AuthError> {
    let cookie = jar
        .get(SESSION_COOKIE)
        .ok_or_else(|| AuthError::Conflict("Not signed in".to_string()))?;

    decode::<SessionClaims>(
        cookie.value(),
        &DecodingKey::from_secret(state.jwt_secret.as_slice()),
        &Validation::new(Algorithm::HS256),
    )
    .map(|token| token.claims)
    .map_err(|_| AuthError::Conflict("Session expired".to_string()))
}

async fn register_impl(
    state: &AuthState,
    request: Credentials,
) -> Result<SessionClaims, AuthError> {
    let email = normalize_email(&request.email)?;
    validate_password(&request.password)?;

    // Serialize registration to prevent two concurrent requests from both
    // passing the "no accounts exist" check (first-user-wins race).
    let _guard = state.register_lock.lock().await;

    let mut accounts = load_accounts(state).await.map_err(AuthError::Storage)?;
    if !accounts.is_empty() {
        return Err(AuthError::Conflict(
            "An account already exists for this deployment".to_string(),
        ));
    }

    let password_hash = hash_password(&request.password).map_err(AuthError::Storage)?;
    accounts.insert(
        email.clone(),
        LocalAccount {
            email: email.clone(),
            password_hash,
            created_at: now_epoch_secs(),
        },
    );
    save_accounts(state, &accounts)
        .await
        .map_err(AuthError::Storage)?;

    Ok(issue_claims(&email))
}

async fn login_impl(state: &AuthState, request: Credentials) -> Result<SessionClaims, AuthError> {
    let email = normalize_email(&request.email)?;
    let accounts = load_accounts(state).await.map_err(AuthError::Storage)?;
    let account = accounts
        .get(&email)
        .ok_or_else(|| AuthError::Conflict("Invalid email or password".to_string()))?;

    verify_password(&request.password, &account.password_hash)
        .map_err(|_| AuthError::Conflict("Invalid email or password".to_string()))?;

    Ok(issue_claims(&email))
}

async fn change_password_impl(
    state: &AuthState,
    email: &str,
    request: ChangePasswordRequest,
) -> Result<(), AuthError> {
    validate_password(&request.new_password)?;

    let mut accounts = load_accounts(state).await.map_err(AuthError::Storage)?;
    let account = accounts
        .get_mut(email)
        .ok_or_else(|| AuthError::Conflict("Account not found".to_string()))?;

    verify_password(&request.current_password, &account.password_hash)
        .map_err(|_| AuthError::Conflict("Current password is incorrect".to_string()))?;

    account.password_hash = hash_password(&request.new_password).map_err(AuthError::Storage)?;
    save_accounts(state, &accounts)
        .await
        .map_err(AuthError::Storage)?;
    Ok(())
}

async fn load_accounts(state: &AuthState) -> Result<BTreeMap<String, LocalAccount>> {
    let raw = state
        .vault
        .get_secret(state.tenant(), ACCOUNTS_SECRET_KEY)
        .or_else(|| {
            if state.tenant() == "default" {
                None
            } else {
                state.vault.get_secret("default", ACCOUNTS_SECRET_KEY)
            }
        });
    if let Some(raw) = raw {
        let accounts = serde_json::from_str(&raw).context("Failed to decode auth accounts")?;
        return Ok(accounts);
    }

    Ok(BTreeMap::new())
}

async fn save_accounts(state: &AuthState, accounts: &BTreeMap<String, LocalAccount>) -> Result<()> {
    let payload = serde_json::to_string(accounts).context("Failed to encode auth accounts")?;
    let _ = state
        .vault
        .cache_secret(state.tenant(), ACCOUNTS_SECRET_KEY, payload.clone());

    let (ciphertext, nonce) = state
        .vault
        .encrypt(payload.as_bytes())
        .map_err(anyhow::Error::msg)
        .context("Failed to encrypt auth accounts")?;
    state
        .storage
        .upsert_secret(state.tenant(), ACCOUNTS_SECRET_KEY, &ciphertext, &nonce)
        .await
        .map_err(anyhow::Error::msg)
        .context("Failed to persist auth accounts")?;

    Ok(())
}

fn normalize_email(email: &str) -> Result<String, AuthError> {
    let normalized = email.trim().to_lowercase();
    if normalized.is_empty() || !normalized.contains('@') {
        return Err(AuthError::Validation(
            "A valid email address is required".to_string(),
        ));
    }
    Ok(normalized)
}

fn validate_password(password: &str) -> Result<(), AuthError> {
    if password.trim().len() < 8 {
        return Err(AuthError::Validation(
            "Password must be at least 8 characters".to_string(),
        ));
    }
    Ok(())
}

fn hash_password(password: &str) -> Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(anyhow::Error::msg)
        .context("Failed to hash password")?
        .to_string())
}

fn verify_password(password: &str, hash: &str) -> Result<()> {
    let parsed = PasswordHash::new(hash)
        .map_err(anyhow::Error::msg)
        .context("Stored password hash is invalid")?;
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .map_err(anyhow::Error::msg)
        .context("Password verification failed")
}

fn issue_claims(email: &str) -> SessionClaims {
    SessionClaims {
        email: email.to_string(),
        provider: "local".to_string(),
        exp: now_epoch_secs() + SESSION_DURATION_SECS,
    }
}

pub(crate) fn issue_session_cookie_value(jwt_secret: &[u8], email: &str) -> Result<String> {
    let token = encode(
        &Header::default(),
        &issue_claims(email),
        &EncodingKey::from_secret(jwt_secret),
    )
    .map_err(anyhow::Error::msg)
    .context("Failed to encode session token")?;

    Ok(format!("{SESSION_COOKIE}={token}"))
}

fn build_session_cookie(state: &AuthState, claims: &SessionClaims) -> Cookie<'static> {
    let token = issue_session_cookie_value(state.jwt_secret.as_slice(), &claims.email)
        .expect("session token should serialize");
    let value = token
        .split_once('=')
        .map(|(_, cookie)| cookie)
        .unwrap_or_default()
        .to_string();

    Cookie::build((SESSION_COOKIE, value))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.cookie_secure)
        .max_age(time::Duration::seconds(SESSION_DURATION_SECS as i64))
        .build()
}

fn now_epoch_secs() -> usize {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_secs() as usize
}

#[derive(Debug)]
enum AuthError {
    Validation(String),
    Conflict(String),
    Storage(anyhow::Error),
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use axum::body::{Body, to_bytes};
    use axum::extract::ConnectInfo;
    use axum::http::HeaderMap;
    use axum::http::{Request, StatusCode};
    use axum::middleware::from_fn_with_state;
    use axum::response::IntoResponse;
    use axum::routing::get;
    use axum::{Json, Router};
    use serde_json::json;
    use tower::ServiceExt;

    use super::{
        AuthState, claims_from_headers, is_public_path, issue_session_cookie_value, middleware,
        router,
    };

    #[test]
    fn webhook_trigger_paths_are_public_for_external_providers() {
        assert!(
            is_public_path("POST", "/triggers/webhook/patrol-datadog"),
            "external webhook providers cannot send Temper bearer auth"
        );
    }

    #[tokio::test]
    async fn register_login_me_and_logout_round_trip() {
        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = router(state);

        let register = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "owner@example.com",
                    "password": "correct horse battery staple"
                })
                .to_string(),
            ))
            .unwrap();

        let register_response = app.clone().oneshot(register).await.unwrap();
        assert_eq!(register_response.status(), StatusCode::CREATED);

        let cookie = register_response
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .expect("register should issue a session cookie")
            .to_string();
        assert!(cookie.contains("paw_session="));

        let me = Request::builder()
            .method("GET")
            .uri("/auth/me")
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap();

        let me_response = app.clone().oneshot(me).await.unwrap();
        assert_eq!(me_response.status(), StatusCode::OK);
        let body = to_bytes(me_response.into_body(), usize::MAX).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("owner@example.com"));

        let logout = Request::builder()
            .method("POST")
            .uri("/auth/logout")
            .header("cookie", &cookie)
            .body(Body::empty())
            .unwrap();

        let logout_response = app.clone().oneshot(logout).await.unwrap();
        assert_eq!(logout_response.status(), StatusCode::OK);

        let anonymous_me = Request::builder()
            .method("GET")
            .uri("/auth/me")
            .body(Body::empty())
            .unwrap();

        let anonymous_response = app.oneshot(anonymous_me).await.unwrap();
        assert_eq!(anonymous_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn register_is_first_user_only() {
        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = router(state);

        let first = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "owner@example.com",
                    "password": "correct horse battery staple"
                })
                .to_string(),
            ))
            .unwrap();

        let first_response = app.clone().oneshot(first).await.unwrap();
        assert_eq!(first_response.status(), StatusCode::CREATED);

        let second = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "another@example.com",
                    "password": "another secret"
                })
                .to_string(),
            ))
            .unwrap();

        let second_response = app.oneshot(second).await.unwrap();
        assert_eq!(second_response.status(), StatusCode::FORBIDDEN);
    }

    #[tokio::test]
    async fn bootstrap_session_cookie_round_trips() {
        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let cookie =
            issue_session_cookie_value(state.jwt_secret.as_slice(), "bootstrap@temperpaw.local")
                .unwrap();

        let mut headers = axum::http::HeaderMap::new();
        headers.insert("cookie", cookie.parse().unwrap());

        let claims = claims_from_headers(&state, &headers).expect("cookie should decode");
        assert_eq!(claims.email, "bootstrap@temperpaw.local");
        assert_eq!(claims.provider, "local");
    }

    #[tokio::test]
    async fn tdata_subpaths_accept_cookie_sessions() {
        async fn tdata_handler(headers: HeaderMap) -> impl IntoResponse {
            let kind = headers
                .get("x-temper-principal-kind")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            let id = headers
                .get("x-temper-principal-id")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            (
                StatusCode::OK,
                Json(json!({
                    "kind": kind,
                    "id": id
                })),
            )
        }

        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = Router::new()
            .route("/tdata/Agents", get(tdata_handler))
            .merge(router(state.clone()))
            .layer(from_fn_with_state(state.clone(), middleware));

        let register = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "owner@example.com",
                    "password": "correct horse battery staple"
                })
                .to_string(),
            ))
            .unwrap();
        let register_response = app.clone().oneshot(register).await.unwrap();
        let cookie = register_response
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .unwrap()
            .to_string();

        let request = Request::builder()
            .method("GET")
            .uri("/tdata/Agents")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap();
        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["kind"], "admin");
        assert_eq!(payload["id"], "owner@example.com");
    }

    #[tokio::test]
    async fn safe_setup_metadata_routes_are_public_only_before_first_account() {
        async fn setup_handler() -> impl IntoResponse {
            StatusCode::OK
        }

        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = Router::new()
            .route("/paw/setup/status", get(setup_handler))
            .merge(router(state.clone()))
            .layer(from_fn_with_state(state.clone(), middleware));

        let bootstrap_request = Request::builder()
            .method("GET")
            .uri("/paw/setup/status")
            .body(Body::empty())
            .unwrap();
        let bootstrap_response = app.clone().oneshot(bootstrap_request).await.unwrap();
        assert_eq!(bootstrap_response.status(), StatusCode::OK);

        let register = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "owner@example.com",
                    "password": "correct horse battery staple"
                })
                .to_string(),
            ))
            .unwrap();
        let register_response = app.clone().oneshot(register).await.unwrap();
        assert_eq!(register_response.status(), StatusCode::CREATED);
        let cookie = register_response
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .unwrap()
            .to_string();

        let locked_request = Request::builder()
            .method("GET")
            .uri("/paw/setup/status")
            .body(Body::empty())
            .unwrap();
        let locked_response = app.clone().oneshot(locked_request).await.unwrap();
        assert_eq!(locked_response.status(), StatusCode::UNAUTHORIZED);

        let authenticated_request = Request::builder()
            .method("GET")
            .uri("/paw/setup/status")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap();
        let authenticated_response = app.oneshot(authenticated_request).await.unwrap();
        assert_eq!(authenticated_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn setup_secret_routes_require_auth_even_before_first_account() {
        async fn setup_handler() -> impl IntoResponse {
            StatusCode::OK
        }

        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = Router::new()
            .route("/paw/setup/secrets/discord_bot_token", get(setup_handler))
            .merge(router(state.clone()))
            .layer(from_fn_with_state(state.clone(), middleware));

        let bootstrap_request = Request::builder()
            .method("GET")
            .uri("/paw/setup/secrets/discord_bot_token")
            .body(Body::empty())
            .unwrap();
        let bootstrap_response = app.clone().oneshot(bootstrap_request).await.unwrap();
        assert_eq!(bootstrap_response.status(), StatusCode::UNAUTHORIZED);

        let register = Request::builder()
            .method("POST")
            .uri("/auth/register")
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "email": "owner@example.com",
                    "password": "correct horse battery staple"
                })
                .to_string(),
            ))
            .unwrap();
        let register_response = app.clone().oneshot(register).await.unwrap();
        assert_eq!(register_response.status(), StatusCode::CREATED);
        let cookie = register_response
            .headers()
            .get("set-cookie")
            .and_then(|value| value.to_str().ok())
            .unwrap()
            .to_string();

        let authenticated_request = Request::builder()
            .method("GET")
            .uri("/paw/setup/secrets/discord_bot_token")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap();
        let authenticated_response = app.oneshot(authenticated_request).await.unwrap();
        assert_eq!(authenticated_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn external_self_asserted_principal_headers_are_rejected() {
        async fn tdata_handler() -> impl IntoResponse {
            StatusCode::OK
        }

        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = Router::new()
            .route("/tdata/Agents", get(tdata_handler))
            .merge(router(state.clone()))
            .layer(from_fn_with_state(state.clone(), middleware));

        // A remote client self-asserting an admin principal, with the real TCP
        // peer being a public (non-loopback) address and no session cookie.
        let mut request = Request::builder()
            .method("GET")
            .uri("/tdata/Agents")
            .header("x-temper-principal-kind", "admin")
            .header("x-temper-principal-id", "attacker")
            .header("x-tenant-id", "default")
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 7], 44321))));

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(
            response.status(),
            StatusCode::UNAUTHORIZED,
            "external self-asserted principal headers must not authenticate as admin"
        );
    }

    #[tokio::test]
    async fn external_forged_identity_family_is_stripped_before_downstream() {
        // A public path passes through the middleware to the handler, so this
        // proves the full x-temper-* family (not just principal id/kind) is
        // removed for external callers before Cedar or a public handler sees it.
        async fn probe_handler(headers: HeaderMap) -> impl IntoResponse {
            let surviving: Vec<String> = headers
                .keys()
                .map(|key| key.as_str().to_string())
                .filter(|key| {
                    key.starts_with("x-temper-") || key == "x-agent-id" || key == "x-tenant-id"
                })
                .collect();
            (StatusCode::OK, Json(json!({ "surviving": surviving })))
        }

        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = Router::new()
            .route("/triggers/webhook/test", get(probe_handler))
            .merge(router(state.clone()))
            .layer(from_fn_with_state(state.clone(), middleware));

        let mut request = Request::builder()
            .method("GET")
            .uri("/triggers/webhook/test")
            .header("x-temper-principal-kind", "admin")
            .header("x-temper-principal-id", "attacker")
            .header("x-temper-agent-role", "admin")
            .header("x-temper-acting-for", "victim")
            .header("x-temper-principal-scopes", "*")
            .header("x-temper-attr-clearance", "top-secret")
            .header("x-temper-ctx-agentTypeVerified", "true")
            .header("x-agent-id", "attacker")
            .header("x-tenant-id", "other-tenant")
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([203, 0, 113, 7], 5555))));

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            payload["surviving"].as_array().map(|values| values.len()),
            Some(0),
            "no client-asserted identity headers may survive to a downstream handler: {payload}"
        );
    }

    #[tokio::test]
    async fn internal_loopback_self_asserted_principal_headers_are_admitted() {
        async fn tdata_handler(headers: HeaderMap) -> impl IntoResponse {
            let kind = headers
                .get("x-temper-principal-kind")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            let id = headers
                .get("x-temper-principal-id")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default();
            (StatusCode::OK, Json(json!({ "kind": kind, "id": id })))
        }

        let tempdir = tempfile::tempdir().unwrap();
        let state = AuthState::for_tests(tempdir.path()).await;
        let app = Router::new()
            .route("/tdata/Agents", get(tdata_handler))
            .merge(router(state.clone()))
            .layer(from_fn_with_state(state.clone(), middleware));

        // A genuinely internal caller: real TCP peer is loopback. Internal
        // transport/setup/startup callers reach the server this way.
        let mut request = Request::builder()
            .method("GET")
            .uri("/tdata/Agents")
            .header("x-temper-principal-kind", "admin")
            .header("x-temper-principal-id", "temperpaw-transport")
            .body(Body::empty())
            .unwrap();
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 44321))));

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload["kind"], "admin");
        assert_eq!(payload["id"], "temperpaw-transport");
    }
}
