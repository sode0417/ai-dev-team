use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use super::model::*;

pub async fn list_projects(pool: &PgPool) -> Result<Vec<ProjectWithRepos>, AppError> {
    let projects: Vec<Project> = sqlx::query_as(
        "SELECT id, name, description, created_at, updated_at FROM projects ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await?;

    let project_ids: Vec<Uuid> = projects.iter().map(|p| p.id).collect();
    let repos: Vec<ProjectRepository> = if project_ids.is_empty() {
        vec![]
    } else {
        sqlx::query_as(
            "SELECT id, project_id, owner, name, default_branch, local_path, created_at FROM project_repositories WHERE project_id = ANY($1) ORDER BY created_at",
        )
        .bind(&project_ids)
        .fetch_all(pool)
        .await?
    };

    let mut repo_map: std::collections::HashMap<Uuid, Vec<ProjectRepository>> =
        std::collections::HashMap::new();
    for repo in repos {
        repo_map.entry(repo.project_id).or_default().push(repo);
    }

    Ok(projects
        .into_iter()
        .map(|p| {
            let repositories = repo_map.remove(&p.id).unwrap_or_default();
            ProjectWithRepos {
                project: p,
                repositories,
            }
        })
        .collect())
}

pub async fn get_project(pool: &PgPool, id: Uuid) -> Result<ProjectWithRepos, AppError> {
    let project: Project = sqlx::query_as(
        "SELECT id, name, description, created_at, updated_at FROM projects WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    let repositories: Vec<ProjectRepository> = sqlx::query_as(
        "SELECT id, project_id, owner, name, default_branch, local_path, created_at FROM project_repositories WHERE project_id = $1 ORDER BY created_at",
    )
    .bind(id)
    .fetch_all(pool)
    .await?;

    Ok(ProjectWithRepos {
        project,
        repositories,
    })
}

pub async fn create_project(pool: &PgPool, req: &CreateProjectRequest) -> Result<Project, AppError> {
    let project: Project = sqlx::query_as(
        "INSERT INTO projects (name, description) VALUES ($1, $2) RETURNING id, name, description, created_at, updated_at",
    )
    .bind(&req.name)
    .bind(&req.description)
    .fetch_one(pool)
    .await?;

    Ok(project)
}

pub async fn update_project(
    pool: &PgPool,
    id: Uuid,
    req: &UpdateProjectRequest,
) -> Result<Project, AppError> {
    let project: Project = sqlx::query_as(
        r#"UPDATE projects SET
            name = COALESCE($2, name),
            description = COALESCE($3, description),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, name, description, created_at, updated_at"#,
    )
    .bind(id)
    .bind(&req.name)
    .bind(&req.description)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(project)
}

pub async fn delete_project(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM projects WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

pub async fn add_repository(
    pool: &PgPool,
    project_id: Uuid,
    req: &AddRepositoryRequest,
) -> Result<ProjectRepository, AppError> {
    // プロジェクト存在確認
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(AppError::NotFound);
    }

    let repo: ProjectRepository = sqlx::query_as(
        r#"INSERT INTO project_repositories (project_id, owner, name, default_branch, local_path)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING id, project_id, owner, name, default_branch, local_path, created_at"#,
    )
    .bind(project_id)
    .bind(&req.owner)
    .bind(&req.name)
    .bind(req.default_branch.as_deref().unwrap_or("main"))
    .bind(&req.local_path)
    .fetch_one(pool)
    .await
    .map_err(|e| match e {
        sqlx::Error::Database(ref db_err) if db_err.constraint().is_some() => {
            AppError::Conflict("Repository already exists for this project".to_string())
        }
        _ => AppError::from(e),
    })?;

    Ok(repo)
}

pub async fn get_repository(
    pool: &PgPool,
    project_id: Uuid,
    repo_id: Uuid,
) -> Result<ProjectRepository, AppError> {
    let repo: ProjectRepository = sqlx::query_as(
        "SELECT id, project_id, owner, name, default_branch, local_path, created_at FROM project_repositories WHERE id = $1 AND project_id = $2",
    )
    .bind(repo_id)
    .bind(project_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)?;

    Ok(repo)
}

pub async fn delete_repository(pool: &PgPool, project_id: Uuid, repo_id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM project_repositories WHERE id = $1 AND project_id = $2")
        .bind(repo_id)
        .bind(project_id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}
