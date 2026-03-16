use axum::extract::{Path, State};
use axum::{Json, Router, routing::{delete, get, post}};
use serde_json::json;
use uuid::Uuid;

use crate::AppState;
use crate::auth::AuthUser;
use crate::error::AppError;
use crate::response::SuccessResponse;
use super::model::*;
use super::service;

async fn list_projects(
    State(state): State<AppState>,
    _auth: AuthUser,
) -> Result<Json<SuccessResponse<Vec<ProjectWithRepos>>>, AppError> {
    let projects = service::list_projects(&state.pool).await?;
    Ok(Json(SuccessResponse {
        data: projects,
        meta: None,
    }))
}

async fn get_project(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<SuccessResponse<ProjectWithRepos>>, AppError> {
    let project = service::get_project(&state.pool, id).await?;
    Ok(Json(SuccessResponse {
        data: project,
        meta: None,
    }))
}

async fn create_project(
    State(state): State<AppState>,
    _auth: AuthUser,
    Json(body): Json<CreateProjectRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = service::create_project(&state.pool, &body).await?;
    Ok(Json(json!({ "data": project })))
}

async fn update_project(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateProjectRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let project = service::update_project(&state.pool, id, &body).await?;
    Ok(Json(json!({ "data": project })))
}

async fn delete_project(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
) -> Result<Json<serde_json::Value>, AppError> {
    service::delete_project(&state.pool, id).await?;
    Ok(Json(json!({ "data": { "id": id, "deleted": true } })))
}

async fn add_repository(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<Uuid>,
    Json(body): Json<AddRepositoryRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let repo = service::add_repository(&state.pool, id, &body).await?;
    Ok(Json(json!({ "data": repo })))
}

async fn delete_repository(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path((project_id, repo_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<serde_json::Value>, AppError> {
    service::delete_repository(&state.pool, project_id, repo_id).await?;
    Ok(Json(json!({ "data": { "deleted": true } })))
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/{id}", get(get_project).put(update_project).delete(delete_project))
        .route("/{id}/repositories", post(add_repository))
        .route("/{id}/repositories/{repo_id}", delete(delete_repository))
}
