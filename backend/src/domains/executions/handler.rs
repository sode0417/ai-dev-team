use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::{Json, Router, routing::get};
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::response::SuccessResponse;
use super::model::*;
use super::service;

async fn list_sessions(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(task_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<Vec<ExecutionSession>>>, AppError> {
    let sessions = service::list_sessions(&state.pool, task_id).await?;
    Ok(Json(SuccessResponse {
        data: sessions,
        meta: None,
    }))
}

async fn list_logs(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(session_id): Path<Uuid>,
    Query(query): Query<ListLogsQuery>,
) -> Result<Json<SuccessResponse<Vec<ExecutionLog>>>, AppError> {
    let logs = service::list_logs(&state.pool, session_id, &query).await?;
    Ok(Json(SuccessResponse {
        data: logs,
        meta: None,
    }))
}

async fn get_screenshot(
    _auth: AuthUser,
    Path((task_id, filename)): Path<(Uuid, String)>,
) -> Result<(HeaderMap, Body), StatusCode> {
    // ファイル名のバリデーション（パストラバーサル防止）
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') {
        return Err(StatusCode::BAD_REQUEST);
    }

    let path = format!("data/qa-screenshots/{task_id}/{filename}");
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;

    let content_type = if filename.ends_with(".png") {
        "image/png"
    } else if filename.ends_with(".jpg") || filename.ends_with(".jpeg") {
        "image/jpeg"
    } else {
        "application/octet-stream"
    };

    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        content_type.parse().expect("static content-type must be valid"),
    );
    headers.insert(
        header::CACHE_CONTROL,
        "public, max-age=86400".parse().expect("static cache-control must be valid"),
    );

    Ok((headers, Body::from(bytes)))
}

pub fn task_routes() -> Router<AppState> {
    Router::new()
        .route("/{task_id}/executions", get(list_sessions))
        .route("/{task_id}/screenshots/{filename}", get(get_screenshot))
}

pub fn execution_routes() -> Router<AppState> {
    Router::new().route("/{id}/logs", get(list_logs))
}
