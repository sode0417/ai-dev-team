use axum::extract::{Path, Query, State};
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

pub fn task_routes() -> Router<AppState> {
    Router::new().route("/{task_id}/executions", get(list_sessions))
}

pub fn execution_routes() -> Router<AppState> {
    Router::new().route("/{id}/logs", get(list_logs))
}
