use axum::extract::{Path, State};
use axum::{Json, Router, routing::get};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::response::SuccessResponse;
use super::model::*;
use super::service;

/// POST /api/projects/{id}/scan — スキャン開始
async fn start_scan(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    // プロジェクト + リポ情報を取得
    let project = crate::domains::projects::service::get_project(&state.pool, project_id).await?;

    if project.repositories.is_empty() {
        return Err(AppError::Validation(
            "Project has no repositories to scan".to_string(),
        ));
    }

    // スキャンセッション作成
    let scan = service::create_scan(&state.pool, project_id).await?;
    let scan_id = scan.id;

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
            scan_id,
            &name,
            desc.as_deref(),
            &repos,
        )
        .await;
    });

    Ok(Json(json!({ "data": { "scan_id": scan_id } })))
}

/// GET /api/projects/{id}/scans — スキャン履歴
async fn list_scans(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(project_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<Vec<ScanSession>>>, AppError> {
    let scans = service::list_scans(&state.pool, project_id).await?;
    Ok(Json(SuccessResponse {
        data: scans,
        meta: None,
    }))
}

/// GET /api/scans/{scan_id} — スキャン結果 + 生成タスク
async fn get_scan_result(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(scan_id): Path<Uuid>,
) -> Result<Json<SuccessResponse<ScanResult>>, AppError> {
    let result = service::get_scan_result(&state.pool, scan_id).await?;
    Ok(Json(SuccessResponse {
        data: result,
        meta: None,
    }))
}

/// プロジェクト配下のスキャンルート（/api/projects にネスト）
pub fn project_routes() -> Router<AppState> {
    Router::new()
        .route("/{id}/scan", axum::routing::post(start_scan))
        .route("/{id}/scans", get(list_scans))
}

/// スキャン単体ルート（/api/scans にネスト）
pub fn scan_routes() -> Router<AppState> {
    Router::new()
        .route("/{scan_id}", get(get_scan_result))
}
