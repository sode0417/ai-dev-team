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
    body: Option<Json<ExecuteTaskRequest>>,
) -> Result<Json<serde_json::Value>, AppError> {
    let skip_hearing = body.as_ref().and_then(|b| b.skip_hearing).unwrap_or(false);

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
    let proposal_type = task.proposal_type.clone();

    if skip_hearing {
        // 即時実行: 既存パイプライン
        let _ = service::update_task_execution(
            &state.pool, id, TaskStatus::Planning, None, None, None, None, None,
        ).await?;

        tokio::spawn(async move {
            crate::executor::pipeline::run_pipeline(
                &pool, &ws_hub, task_id, &task_title, &task_description, &local_path, &branch, &proposal_type,
            ).await;
        });
    } else {
        // ヒアリング付き実行
        let _ = service::update_task_execution(
            &state.pool, id, TaskStatus::Hearing, None, None, None, None, None,
        ).await?;

        tokio::spawn(async move {
            crate::executor::pipeline::run_hearing_phase(
                &pool, &ws_hub, task_id, &task_title, &task_description, &local_path, &branch, &proposal_type,
            ).await;
        });
    }

    let task = service::get_task(&state.pool, id).await?;
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

// === ヒアリング・計画承認エンドポイント ===

async fn list_hearings(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let hearings = service::list_hearings(&state.pool, id).await?;
    Ok(Json(json!({ "data": hearings })))
}

async fn answer_hearing(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AnswerHearingRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = service::get_task(&state.pool, id).await?;
    if task.status != TaskStatus::Hearing {
        return Err(AppError::Validation(format!(
            "Task must be in 'hearing' status, got '{:?}'",
            task.status
        )));
    }

    // 最新のヒアリングを取得
    let hearing = service::get_latest_hearing(&state.pool, id).await?
        .ok_or_else(|| AppError::Validation("No pending hearing found".to_string()))?;

    if hearing.status != "pending" {
        return Err(AppError::Validation("Hearing already answered".to_string()));
    }

    // 回答を保存
    let answers_json = serde_json::to_value(&body.answers).unwrap_or_default();
    let hearing = service::answer_hearing(&state.pool, hearing.id, &answers_json).await?;

    // リポ情報取得（planning / re-hearing 用）
    let repo_id = task.repository_id.ok_or_else(|| {
        AppError::Validation("Task must have a repository".to_string())
    })?;
    let repo: crate::domains::projects::model::ProjectRepository = sqlx::query_as(
        "SELECT id, project_id, owner, name, default_branch, local_path, created_at \
         FROM project_repositories WHERE id = $1",
    )
    .bind(repo_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let local_path = repo.local_path.clone().unwrap_or_default();

    // 次のフェーズを決定
    let pool = state.pool.clone();
    let ws_hub = state.ws_hub.clone();
    let task_id = task.id;
    let task_title = task.title.clone();
    let task_description = task.description.clone();
    let proposal_type = task.proposal_type.clone();
    let phase = hearing.phase.clone();

    if phase == "pre_plan" {
        // 計画フェーズへ
        tokio::spawn(async move {
            crate::executor::pipeline::run_planning_phase(
                &pool, &ws_hub, task_id, &task_title, &task_description, &local_path, &proposal_type,
            ).await;
        });
    } else {
        // in_plan: 回答を反映して再計画
        tokio::spawn(async move {
            crate::executor::pipeline::run_planning_phase(
                &pool, &ws_hub, task_id, &task_title, &task_description, &local_path, &proposal_type,
            ).await;
        });
    }

    let task = service::get_task(&state.pool, id).await?;
    Ok(Json(json!({ "data": task, "hearing": hearing })))
}

async fn approve_plan(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = service::approve_plan(&state.pool, id).await?;

    // リポ情報取得
    let repo_id = task.repository_id.ok_or_else(|| {
        AppError::Validation("Task must have a repository".to_string())
    })?;
    let repo: crate::domains::projects::model::ProjectRepository = sqlx::query_as(
        "SELECT id, project_id, owner, name, default_branch, local_path, created_at \
         FROM project_repositories WHERE id = $1",
    )
    .bind(repo_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(AppError::NotFound)?;
    let local_path = repo.local_path.clone().unwrap_or_default();

    // 実行フェーズを開始
    let pool = state.pool.clone();
    let ws_hub = state.ws_hub.clone();
    let task_id = task.id;
    let task_title = task.title.clone();
    let task_description = task.description.clone();
    let proposal_type = task.proposal_type.clone();

    tokio::spawn(async move {
        crate::executor::pipeline::run_execution_phase(
            &pool, &ws_hub, task_id, &task_title, &task_description, &local_path, &proposal_type,
        ).await;
    });

    Ok(Json(json!({ "data": task })))
}

async fn reject_plan(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<RejectPlanRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let task = service::reject_plan(&state.pool, id, &body.action).await?;

    if body.action == "replan" {
        // フィードバックをヒアリング回答として追加
        if let Some(ref feedback) = body.feedback {
            let questions = serde_json::json!([{
                "index": 1,
                "question": "計画へのフィードバック"
            }]);
            let hearing = service::create_hearing(&state.pool, id, None, "in_plan", 1, &questions).await?;
            let answers = serde_json::json!([{
                "index": 1,
                "answer": feedback
            }]);
            let _ = service::answer_hearing(&state.pool, hearing.id, &answers).await?;
        }

        // 再計画
        let repo_id = task.repository_id.ok_or_else(|| {
            AppError::Validation("Task must have a repository".to_string())
        })?;
        let repo: crate::domains::projects::model::ProjectRepository = sqlx::query_as(
            "SELECT id, project_id, owner, name, default_branch, local_path, created_at \
             FROM project_repositories WHERE id = $1",
        )
        .bind(repo_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(AppError::NotFound)?;
        let local_path = repo.local_path.clone().unwrap_or_default();

        let pool = state.pool.clone();
        let ws_hub = state.ws_hub.clone();
        let task_id = task.id;
        let task_title = task.title.clone();
        let task_description = task.description.clone();
        let proposal_type = task.proposal_type.clone();

        tokio::spawn(async move {
            crate::executor::pipeline::run_planning_phase(
                &pool, &ws_hub, task_id, &task_title, &task_description, &local_path, &proposal_type,
            ).await;
        });
    }

    let task = service::get_task(&state.pool, id).await?;
    Ok(Json(json!({ "data": task })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_tasks).post(create_task))
        .route("/{id}", get(get_task).put(update_task))
        .route("/{id}/approve", post(approve_task))
        .route("/{id}/execute", post(execute_task))
        .route("/{id}/cancel", post(cancel_task))
        .route("/{id}/hearings", get(list_hearings))
        .route("/{id}/hearing/answer", post(answer_hearing))
        .route("/{id}/approve-plan", post(approve_plan))
        .route("/{id}/reject-plan", post(reject_plan))
}
