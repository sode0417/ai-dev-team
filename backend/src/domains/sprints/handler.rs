use axum::extract::{Path, State};
use axum::{Json, Router, routing::{get, post}};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::response::SuccessResponse;
use super::model::*;
use super::service;

#[derive(Debug, Deserialize)]
struct ApprovePlanRequest {
    max_parallel_tasks: Option<i32>,
}

/// POST /api/projects/{id}/sprints — スプリント作成 + スキャン開始
async fn create_sprint(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = crate::domains::projects::service::get_project(&state.pool, project_id).await?;

    if project.repositories.is_empty() {
        return Err(AppError::Validation(
            "Project has no repositories to scan".to_string(),
        ));
    }

    let sprint = service::create_sprint(&state.pool, project_id).await?;
    let sprint_id = sprint.id;

    // バックグラウンドでスキャン実行
    let pool = state.pool.clone();
    let ws_hub = state.ws_hub.clone();
    let github = state.github.clone();
    let name = project.project.name.clone();
    let desc = project.project.description.clone();
    let repos = project.repositories.clone();

    tokio::spawn(async move {
        crate::scanner::analyzer::run_scan(
            &pool,
            &ws_hub,
            &github,
            project_id,
            sprint_id,
            &name,
            desc.as_deref(),
            &repos,
        )
        .await;
    });

    Ok(Json(json!({ "data": { "sprint_id": sprint_id } })))
}

/// GET /api/projects/{id}/sprints — スプリント一覧
async fn list_sprints(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<Vec<Sprint>>>, AppError> {
    let sprints = service::list_sprints(&state.pool, project_id).await?;
    Ok(Json(SuccessResponse {
        data: sprints,
        meta: None,
    }))
}

/// GET /api/projects/{id}/sprint/active — アクティブスプリント
async fn get_active_sprint(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_active_sprint(&state.pool, project_id).await?;
    match sprint {
        Some(s) => {
            let tasks = service::get_sprint_tasks(&state.pool, s.id).await?;
            Ok(Json(json!({ "data": SprintWithTasks { sprint: s, tasks } })))
        }
        None => Ok(Json(json!({ "data": null }))),
    }
}

/// GET /api/sprints/{id} — スプリント詳細 + タスク一覧
async fn get_sprint(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<SprintWithTasks>>, AppError> {
    let result = service::get_sprint_with_tasks(&state.pool, sprint_id).await?;
    Ok(Json(SuccessResponse {
        data: result,
        meta: None,
    }))
}

/// POST /api/sprints/{id}/select-tasks — タスク選定 (採用/却下)
async fn select_tasks(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
    Json(body): Json<SelectTasksRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_sprint(&state.pool, sprint_id).await?;
    if SprintStatus::from_str(&sprint.status) != SprintStatus::Selecting {
        return Err(AppError::Validation(
            "Sprint must be in 'selecting' status".to_string(),
        ));
    }

    let tasks = service::select_tasks(
        &state.pool,
        sprint_id,
        &body.approved_task_ids,
        &body.rejected_task_ids,
    )
    .await?;

    Ok(Json(json!({ "data": tasks })))
}

/// POST /api/sprints/{id}/start-hearing — ヒアリング開始
/// 選定済みタスクの hearing を順次開始する
async fn start_hearing(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_sprint(&state.pool, sprint_id).await?;
    if SprintStatus::from_str(&sprint.status) != SprintStatus::Selecting {
        return Err(AppError::Validation(
            "Sprint must be in 'selecting' status to start hearing".to_string(),
        ));
    }

    // 承認済みタスクがあるか確認
    let tasks = service::get_sprint_tasks(&state.pool, sprint_id).await?;
    let approved: Vec<_> = tasks
        .iter()
        .filter(|t| t.status == crate::domains::tasks::model::TaskStatus::Approved)
        .collect();

    if approved.is_empty() {
        return Err(AppError::Validation(
            "No approved tasks to start hearing".to_string(),
        ));
    }

    // スプリントを hearing に
    let sprint = service::update_status(&state.pool, sprint_id, "hearing").await?;

    // 各タスクのヒアリングをバックグラウンドで開始
    for task in &approved {
        let repo_id = match task.repository_id {
            Some(id) => id,
            None => continue,
        };

        let repo: Option<crate::domains::projects::model::ProjectRepository> = sqlx::query_as(
            "SELECT id, project_id, owner, name, default_branch, local_path, created_at \
             FROM project_repositories WHERE id = $1",
        )
        .bind(repo_id)
        .fetch_optional(&state.pool)
        .await?;

        let repo = match repo {
            Some(r) => r,
            None => continue,
        };

        let local_path = match repo.local_path {
            Some(p) => p,
            None => continue,
        };

        let default_branch = repo.default_branch.clone();

        // タスクを hearing 状態に
        crate::domains::tasks::service::update_task_execution(
            &state.pool,
            task.id,
            crate::domains::tasks::model::TaskStatus::Hearing,
            None, None, None, None, None,
        )
        .await?;

        let pool = state.pool.clone();
        let ws_hub = state.ws_hub.clone();
        let task_id = task.id;
        let task_title = task.title.clone();
        let task_description = task.description.clone();
        let branch = default_branch;
        let proposal_type = task.proposal_type.clone();

        tokio::spawn(async move {
            crate::executor::pipeline::run_hearing_phase(
                &pool, &ws_hub, task_id, &task_title, &task_description,
                &local_path, &branch, &proposal_type,
            )
            .await;
        });
    }

    Ok(Json(json!({ "data": sprint })))
}

/// GET /api/sprints/{id}/readiness — 全タスクのヒアリング完了状態を確認
async fn check_readiness(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let all_ready = service::all_tasks_ready(&state.pool, sprint_id).await?;
    let tasks = service::get_sprint_tasks(&state.pool, sprint_id).await?;

    let task_statuses: Vec<_> = tasks
        .iter()
        .filter(|t| t.status != crate::domains::tasks::model::TaskStatus::Cancelled)
        .map(|t| json!({
            "id": t.id,
            "title": t.title,
            "status": t.status,
        }))
        .collect();

    Ok(Json(json!({
        "data": {
            "all_ready": all_ready,
            "tasks": task_statuses,
        }
    })))
}

/// POST /api/sprints/{id}/plan — PM Agent に実行順序計画を依頼
async fn create_plan(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_sprint(&state.pool, sprint_id).await?;
    let status = SprintStatus::from_str(&sprint.status);

    // hearing → planning 初回、planning → planning リトライ（計画未生成時）
    if status == SprintStatus::Hearing {
        let all_ready = service::all_tasks_ready(&state.pool, sprint_id).await?;
        if !all_ready {
            return Err(AppError::Validation(
                "Not all tasks have completed hearing".to_string(),
            ));
        }
        // planning に遷移
        service::update_status(&state.pool, sprint_id, "planning").await?;
    } else if status == SprintStatus::Planning {
        // planning リトライ: execution_plan が未生成の場合のみ許可
        if sprint.execution_plan.is_some() {
            return Err(AppError::Validation(
                "Plan already exists. Use approve-plan to proceed.".to_string(),
            ));
        }
    } else {
        return Err(AppError::Validation(
            "Sprint must be in 'hearing' or 'planning' status to plan".to_string(),
        ));
    }

    // バックグラウンドで実行計画を作成
    let pool = state.pool.clone();
    let ws_hub = state.ws_hub.clone();

    tokio::spawn(async move {
        crate::scanner::analyzer::run_sprint_planning(&pool, &ws_hub, sprint_id).await;
    });

    Ok(Json(json!({ "data": sprint })))
}

/// POST /api/sprints/{id}/approve-plan — 実行計画承認 → 実行開始
async fn approve_plan(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_sprint(&state.pool, sprint_id).await?;
    if SprintStatus::from_str(&sprint.status) != SprintStatus::Planning {
        return Err(AppError::Validation(
            "Sprint must be in 'planning' status to approve".to_string(),
        ));
    }

    // executing に遷移（execution_plan は planning フェーズで既に設定済み）
    let plan = sprint.execution_plan.clone().unwrap_or_default();
    let sprint = service::approve_plan(&state.pool, sprint_id, &plan).await?;

    // バックグラウンドでスプリント実行
    let pool = state.pool.clone();
    let ws_hub = state.ws_hub.clone();

    tokio::spawn(async move {
        crate::scanner::analyzer::run_sprint_execution(&pool, &ws_hub, sprint_id).await;
    });

    Ok(Json(json!({ "data": sprint })))
}

/// POST /api/sprints/{id}/cancel — スプリント強制キャンセル
async fn cancel_sprint(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_sprint(&state.pool, sprint_id).await?;
    let status = SprintStatus::from_str(&sprint.status);

    // 完了済み・失敗済みはキャンセル不可
    if status == SprintStatus::Completed || status == SprintStatus::Failed {
        return Err(AppError::Validation(
            "Cannot cancel a completed or failed sprint".to_string(),
        ));
    }

    // 紐づくタスクのうち進行中のものを cancelled に
    let tasks = service::get_sprint_tasks(&state.pool, sprint_id).await?;
    for task in &tasks {
        if task.status != crate::domains::tasks::model::TaskStatus::Completed
            && task.status != crate::domains::tasks::model::TaskStatus::Failed
            && task.status != crate::domains::tasks::model::TaskStatus::Cancelled
        {
            let _ = crate::domains::tasks::service::update_task_execution(
                &state.pool,
                task.id,
                crate::domains::tasks::model::TaskStatus::Cancelled,
                None, None, None, None, None,
            )
            .await;
        }
    }

    // スプリントを failed に
    let sprint = service::fail_sprint(
        &state.pool,
        sprint_id,
        "ユーザーによるキャンセル",
    )
    .await?;

    Ok(Json(json!({ "data": sprint })))
}

/// POST /api/sprints/{id}/feedback — ユーザーフィードバック → improving or completed
async fn submit_feedback(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
    Json(body): Json<SprintFeedbackRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_sprint(&state.pool, sprint_id).await?;
    if SprintStatus::from_str(&sprint.status) != SprintStatus::Retrospective {
        return Err(AppError::Validation(
            "Sprint must be in 'retrospective' status to submit feedback".to_string(),
        ));
    }

    let sprint = service::start_improving(&state.pool, sprint_id, &body.feedback).await?;

    // improving に遷移した場合、バックグラウンドで改善フェーズを実行
    if SprintStatus::from_str(&sprint.status) == SprintStatus::Improving {
        let pool = state.pool.clone();
        let ws_hub = state.ws_hub.clone();
        let github = state.github.clone();
        let sid = sprint_id;

        tokio::spawn(async move {
            crate::scanner::analyzer::run_improving_phase(&pool, &ws_hub, &github, sid).await;
        });
        // Note: run_improving_phase handles all errors internally (saves to DB + broadcasts).
        // Panics are logged by tokio's default panic hook.
    }

    Ok(Json(json!({ "data": sprint })))
}

/// POST /api/sprints/{id}/complete — improving フェーズ完了後にスプリント完了
async fn complete_sprint(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(sprint_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    let sprint = service::get_sprint(&state.pool, sprint_id).await?;
    if SprintStatus::from_str(&sprint.status) != SprintStatus::Improving {
        return Err(AppError::Validation(
            "Sprint must be in 'improving' status to complete".to_string(),
        ));
    }

    let sprint = service::complete_after_improving(&state.pool, sprint_id).await?;

    Ok(Json(json!({ "data": sprint })))
}

/// プロジェクト配下のスプリントルート
pub fn project_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/sprints", get(list_sprints).post(create_sprint))
        .route("/{id}/sprint/active", get(get_active_sprint))
}

/// スプリント単体ルート
pub fn sprint_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}", get(get_sprint))
        .route("/{id}/select-tasks", post(select_tasks))
        .route("/{id}/start-hearing", post(start_hearing))
        .route("/{id}/readiness", get(check_readiness))
        .route("/{id}/plan", post(create_plan))
        .route("/{id}/approve-plan", post(approve_plan))
        .route("/{id}/cancel", post(cancel_sprint))
        .route("/{id}/feedback", post(submit_feedback))
        .route("/{id}/complete", post(complete_sprint))
}
