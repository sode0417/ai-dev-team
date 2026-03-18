use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use axum::extract::{Request, State};
use axum::middleware::{self, Next};
use axum::response::IntoResponse;
use axum::{Json, Router};
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
        .unwrap()
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

    login_route.route("/refresh", axum::routing::post(refresh))
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
