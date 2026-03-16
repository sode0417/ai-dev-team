use axum::extract::{Path, Query, State};
use axum::{Json, Router, routing::{get, post}};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::response::SuccessResponse;
use super::model::*;
use super::service;

async fn list_tasks(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(query): Query<ListTasksQuery>,
) -> Result<Json<SuccessResponse<Vec<Task>>>, AppError> {
    let tasks = service::list_tasks(&state.pool, &query).await?;
    Ok(Json(SuccessResponse {
        data: tasks,
        meta: None,
    }))
}

async fn get_task(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<Task>>, AppError> {
    let task = service::get_task(&state.pool, id).await?;
    Ok(Json(SuccessResponse {
        data: task,
        meta: None,
    }))
}

async fn create_task(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<CreateTaskRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = service::create_task(&state.pool, &body).await?;
    Ok(Json(json!({ "data": task })))
}

async fn update_task(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateTaskRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = service::update_task(&state.pool, id, &body).await?;
    Ok(Json(json!({ "data": task })))
}

async fn approve_task(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = service::approve_task(&state.pool, id).await?;
    Ok(Json(json!({ "data": task })))
}

async fn execute_task(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // タスクの状態を確認して実行開始
    let task = service::get_task(&state.pool, id).await?;
    if task.status != TaskStatus::Proposed && task.status != TaskStatus::Approved {
        return Err(AppError::Validation(format!(
            "Task must be 'proposed' or 'approved' to execute, got '{:?}'",
            task.status
        )));
    }

    // リポジトリ情報を取得
    let repo_id = task.repository_id.ok_or_else(|| {
        AppError::Validation("Task must have a repository to execute".to_string())
    })?;

    let repo: crate::domains::projects::model::ProjectRepository = sqlx::query_as::<_, crate::domains::projects::model::ProjectRepository>(
        "SELECT id, project_id, owner, name, default_branch, local_path, created_at \
         FROM project_repositories WHERE id = $1",
    )
    .bind(repo_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let local_path = repo.local_path.ok_or_else(|| {
        AppError::Validation("Repository must have a local_path configured".to_string())
    })?;

    // バックグラウンドで実行開始
    let pool = state.pool.clone();
    let ws_hub = state.ws_hub.clone();
    let task_id = task.id;
    let task_title = task.title.clone();
    let task_description = task.description.clone();
    let branch = repo.default_branch.clone();

    tokio::spawn(async move {
        crate::executor::pipeline::run_pipeline(
            &pool,
            &ws_hub,
            task_id,
            &task_title,
            &task_description,
            &local_path,
            &branch,
        )
        .await;
    });

    // ステータスを planning に更新
    let task = service::update_task_execution(
        &state.pool,
        id,
        TaskStatus::Planning,
        None,
        None,
        None,
        None,
        None,
    )
    .await?;

    Ok(Json(json!({ "data": task })))
}

async fn cancel_task(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = service::cancel_task(&state.pool, id).await?;
    Ok(Json(json!({ "data": task })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_tasks).post(create_task))
        .route("/{id}", get(get_task).put(update_task))
        .route("/{id}/approve", post(approve_task))
        .route("/{id}/execute", post(execute_task))
        .route("/{id}/cancel", post(cancel_task))
}
