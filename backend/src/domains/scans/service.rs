use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use super::model::*;
use crate::domains::tasks::model::{Task, TaskPriority};

const SCAN_COLS: &str = "id, project_id, status, analysis, priority_actions, \
    retrospective, improvement_suggestions, error_log, started_at, completed_at";

const TASK_COLS: &str = "id, project_id, repository_id, title, description, status, priority, \
    depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
    retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, \
    scan_id, proposal_type, sprint_id";

pub async fn create_scan(pool: &PgPool, project_id: Uuid) -> Result<ScanSession, AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(AppError::Validation("Project not found".to_string()));
    }

    sqlx::query_as::<_, ScanSession>(
        &format!(
            "INSERT INTO scan_sessions (project_id) VALUES ($1) RETURNING {SCAN_COLS}"
        ),
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_scan_completed(
    pool: &PgPool,
    scan_id: Uuid,
    analysis: &str,
    priority_actions: &serde_json::Value,
    retrospective: Option<&str>,
    improvement_suggestions: Option<&serde_json::Value>,
) -> Result<ScanSession, AppError> {
    sqlx::query_as::<_, ScanSession>(
        &format!(
            "UPDATE scan_sessions SET \
                status = 'completed', \
                analysis = $2, \
                priority_actions = $3, \
                retrospective = $4, \
                improvement_suggestions = $5, \
                completed_at = NOW() \
            WHERE id = $1 RETURNING {SCAN_COLS}"
        ),
    )
    .bind(scan_id)
    .bind(analysis)
    .bind(priority_actions)
    .bind(retrospective)
    .bind(improvement_suggestions)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_scan_failed(
    pool: &PgPool,
    scan_id: Uuid,
    error_log: &str,
) -> Result<ScanSession, AppError> {
    sqlx::query_as::<_, ScanSession>(
        &format!(
            "UPDATE scan_sessions SET \
                status = 'failed', \
                error_log = $2, \
                completed_at = NOW() \
            WHERE id = $1 RETURNING {SCAN_COLS}"
        ),
    )
    .bind(scan_id)
    .bind(error_log)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn get_scan(pool: &PgPool, scan_id: Uuid) -> Result<ScanSession, AppError> {
    sqlx::query_as::<_, ScanSession>(
        &format!("SELECT {SCAN_COLS} FROM scan_sessions WHERE id = $1"),
    )
    .bind(scan_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn list_scans(pool: &PgPool, project_id: Uuid) -> Result<Vec<ScanSession>, AppError> {
    Ok(sqlx::query_as::<_, ScanSession>(
        &format!(
            "SELECT {SCAN_COLS} FROM scan_sessions WHERE project_id = $1 \
             ORDER BY started_at DESC LIMIT 20"
        ),
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?)
}

pub async fn get_scan_result(pool: &PgPool, scan_id: Uuid) -> Result<ScanResult, AppError> {
    let session = get_scan(pool, scan_id).await?;
    let tasks = sqlx::query_as::<_, Task>(
        &format!("SELECT {TASK_COLS} FROM tasks WHERE scan_id = $1 ORDER BY created_at"),
    )
    .bind(scan_id)
    .fetch_all(pool)
    .await?;

    Ok(ScanResult { session, tasks })
}

/// スキャン結果からタスクを一括作成
pub async fn create_tasks_from_proposals(
    pool: &PgPool,
    project_id: Uuid,
    scan_id: Uuid,
    proposals: &[TaskProposal],
    repo_lookup: &std::collections::HashMap<String, Uuid>,
    sprint_id: Option<Uuid>,
) -> Result<Vec<Task>, AppError> {
    let mut created = Vec::new();

    for (i, proposal) in proposals.iter().enumerate() {
        let repository_id = proposal
            .repository_name
            .as_deref()
            .and_then(|name| repo_lookup.get(name).copied());

        let priority = match proposal.priority.as_deref() {
            Some("critical") => TaskPriority::Critical,
            Some("high") => TaskPriority::High,
            Some("low") => TaskPriority::Low,
            _ => TaskPriority::Medium,
        };

        let proposal_type = proposal
            .proposal_type
            .as_deref()
            .unwrap_or("development");

        let task = sqlx::query_as::<_, Task>(
            &format!(
                "INSERT INTO tasks \
                    (project_id, repository_id, title, description, priority, \
                     proposed_by, execution_order, scan_id, proposal_type, sprint_id) \
                 VALUES ($1, $2, $3, $4, $5, 'scan', $6, $7, $8, $9) \
                 RETURNING {TASK_COLS}"
            ),
        )
        .bind(project_id)
        .bind(repository_id)
        .bind(&proposal.title)
        .bind(&proposal.description)
        .bind(&priority)
        .bind(i as i32)
        .bind(scan_id)
        .bind(proposal_type)
        .bind(sprint_id)
        .fetch_one(pool)
        .await?;

        created.push(task);
    }

    Ok(created)
}

/// 振り返り用: 直近の完了/失敗タスク取得
pub async fn get_recent_completed_tasks(
    pool: &PgPool,
    project_id: Uuid,
    limit: i64,
) -> Result<Vec<Task>, AppError> {
    Ok(sqlx::query_as::<_, Task>(
        &format!(
            "SELECT {TASK_COLS} FROM tasks \
             WHERE project_id = $1 AND status IN ('completed', 'failed') \
             ORDER BY completed_at DESC NULLS LAST LIMIT $2"
        ),
    )
    .bind(project_id)
    .bind(limit)
    .fetch_all(pool)
    .await?)
}

/// 前回のスキャンセッション取得
pub async fn get_last_scan(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<ScanSession>, AppError> {
    Ok(sqlx::query_as::<_, ScanSession>(
        &format!(
            "SELECT {SCAN_COLS} FROM scan_sessions \
             WHERE project_id = $1 AND status = 'completed' \
             ORDER BY completed_at DESC LIMIT 1"
        ),
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?)
}
