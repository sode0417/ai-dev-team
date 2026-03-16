use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProjectRepository {
    pub id: Uuid,
    pub project_id: Uuid,
    pub owner: String,
    pub name: String,
    pub default_branch: String,
    pub local_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProjectWithRepos {
    #[serde(flatten)]
    pub project: Project,
    pub repositories: Vec<ProjectRepository>,
}

#[derive(Debug, Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateProjectRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddRepositoryRequest {
    pub owner: String,
    pub name: String,
    pub default_branch: Option<String>,
    pub local_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GitHubListParams {
    pub state: Option<String>,
    pub page: Option<u32>,
    pub per_page: Option<u32>,
}
