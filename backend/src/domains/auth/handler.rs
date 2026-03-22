use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::{Query, Request, State};
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Redirect};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, SameSite};
use axum_extra::extract::CookieJar;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthUser;
use crate::error::AppError;
use crate::AppState;

use super::model::{LoginRequest, RefreshRequest};
use super::service;

/// ブルートフォース対策: スライディングウィンドウ方式のレート制限
#[derive(Clone)]
struct LoginRateLimit {
    count: Arc<AtomicU64>,
    window_start: Arc<AtomicU64>,
}

impl LoginRateLimit {
    fn new() -> Self {
        Self {
            count: Arc::new(AtomicU64::new(0)),
            window_start: Arc::new(AtomicU64::new(0)),
        }
    }
}

async fn login_rate_limit(
    State(limiter): State<LoginRateLimit>,
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, AppError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock must not be before UNIX epoch")
        .as_secs();

    let window = limiter.window_start.load(Ordering::Relaxed);
    if now - window >= 10 {
        limiter.window_start.store(now, Ordering::Relaxed);
        limiter.count.store(1, Ordering::Relaxed);
    } else {
        let count = limiter.count.fetch_add(1, Ordering::Relaxed) + 1;
        if count > 10 {
            return Err(AppError::Validation(
                "Too many login attempts. Please try again later.".to_string(),
            ));
        }
    }

    Ok(next.run(request).await)
}

pub fn public_routes() -> Router<AppState> {
    let limiter = LoginRateLimit::new();

    let login_route = Router::new()
        .route("/login", axum::routing::post(login))
        .route_layer(middleware::from_fn_with_state(limiter, login_rate_limit));

    login_route
        .route("/refresh", axum::routing::post(refresh))
        .route("/google", axum::routing::get(google_redirect))
        .route("/google/callback", axum::routing::get(google_callback))
        .route("/logout", axum::routing::post(logout))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new().route("/me", axum::routing::get(me))
}

async fn login(
    State(state): State<AppState>,
    Json(body): Json<LoginRequest>,
) -> Result<Json<Value>, AppError> {
    let auth = service::login(&state.pool, &state.config, &body.username, &body.password).await?;
    Ok(Json(json!({ "data": auth })))
}

async fn refresh(
    State(state): State<AppState>,
    Json(body): Json<RefreshRequest>,
) -> Result<Json<Value>, AppError> {
    let auth = service::refresh(&state.pool, &state.config, &body.refresh_token).await?;
    Ok(Json(json!({ "data": auth })))
}

async fn me(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Value>, AppError> {
    let user = service::get_me(&state.pool, auth.user_id).await?;
    Ok(Json(json!({ "data": user })))
}

// --- Google OAuth ---

async fn google_redirect(State(state): State<AppState>) -> Result<Redirect, AppError> {
    let config = &state.config;
    if config.google_client_id.is_empty() {
        return Err(AppError::Internal(
            "Google OAuth not configured".to_string(),
        ));
    }

    let auth_url = format!(
        "https://accounts.google.com/o/oauth2/v2/auth?\
         client_id={}&\
         redirect_uri={}&\
         response_type=code&\
         scope=openid%20email%20profile&\
         access_type=offline",
        urlencod(&config.google_client_id),
        urlencod(&config.google_callback_url),
    );

    Ok(Redirect::temporary(&auth_url))
}

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: String,
}

#[derive(Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
}

#[derive(Deserialize)]
struct GoogleUserInfo {
    email: String,
    name: Option<String>,
}

async fn google_callback(
    State(state): State<AppState>,
    Query(query): Query<OAuthCallbackQuery>,
    jar: CookieJar,
) -> Result<(CookieJar, Redirect), AppError> {
    let config = &state.config;
    let http = reqwest::Client::new();

    // 1. Authorization code → Access Token
    let token_resp = http
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("code", query.code.as_str()),
            ("client_id", config.google_client_id.as_str()),
            ("client_secret", config.google_client_secret.as_str()),
            ("redirect_uri", config.google_callback_url.as_str()),
            ("grant_type", "authorization_code"),
        ])
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("Token exchange failed: {e}")))?;

    if !token_resp.status().is_success() {
        let body = token_resp.text().await.unwrap_or_default();
        return Err(AppError::Internal(format!("Google token exchange failed: {body}")));
    }

    let token_data: GoogleTokenResponse = token_resp.json().await
        .map_err(|e| AppError::Internal(format!("Token parse failed: {e}")))?;

    // 2. Access Token → ユーザー情報
    let user_info: GoogleUserInfo = http
        .get("https://www.googleapis.com/oauth2/v2/userinfo")
        .header("Authorization", format!("Bearer {}", token_data.access_token))
        .send()
        .await
        .map_err(|e| AppError::Internal(format!("User info failed: {e}")))?
        .json()
        .await
        .map_err(|e| AppError::Internal(format!("User info parse failed: {e}")))?;

    // 3. ユーザー upsert
    let user = service::upsert_google_user(
        &state.pool,
        &user_info.email,
        user_info.name.as_deref(),
    )
    .await?;

    // 4. JWT 発行
    let jwt = service::generate_access_token(user.id, &user.username, config)?;

    // 5. Cookie 設定
    let mut cookie = Cookie::new("f2a_token", jwt);
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_max_age(time::Duration::seconds(config.jwt_access_expiry_secs));
    if !config.cookie_domain.is_empty() {
        cookie.set_domain(config.cookie_domain.clone());
    }

    Ok((jar.add(cookie), Redirect::temporary(&config.web_url)))
}

async fn logout(jar: CookieJar) -> (CookieJar, Json<Value>) {
    let mut cookie = Cookie::new("f2a_token", "");
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::ZERO);
    (jar.remove(cookie), Json(json!({ "data": { "logged_out": true } })))
}

fn urlencod(s: &str) -> String {
    s.replace('&', "%26").replace('=', "%3D").replace('+', "%2B").replace(' ', "%20")
}
