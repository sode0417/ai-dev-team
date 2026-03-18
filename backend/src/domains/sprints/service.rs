use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use super::model::*;
use crate::domains::tasks::model::{Task, TaskStatus};

const SPRINT_COLS: &str = "id, project_id, status, scan_analysis, priority_actions, \
    execution_plan, retrospective, improvement_suggestions, user_feedback, \
    max_parallel_tasks, error_log, created_at, started_at, completed_at";

const TASK_COLS: &str = "id, project_id, repository_id, title, description, status, priority, \
    depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
    retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, \
    scan_id, proposal_type, sprint_id";

/// スプリント作成
pub async fn create_sprint(pool: &PgPool, project_id: Uuid) -> Result<Sprint, AppError> {
    // プロジェクト存在確認
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(project_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(AppError::Validation("Project not found".to_string()));
    }

    // 同一プロジェクトで進行中のスプリントがないか確認
    let active: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM sprints WHERE project_id = $1 AND status NOT IN ('completed', 'failed'))",
    )
    .bind(project_id)
    .fetch_one(pool)
    .await?;
    if active {
        return Err(AppError::Validation(
            "Project already has an active sprint".to_string(),
        ));
    }

    sqlx::query_as::<_, Sprint>(
        &format!("INSERT INTO sprints (project_id) VALUES ($1) RETURNING {SPRINT_COLS}"),
    )
    .bind(project_id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// スプリント取得
pub async fn get_sprint(pool: &PgPool, sprint_id: Uuid) -> Result<Sprint, AppError> {
    sqlx::query_as::<_, Sprint>(
        &format!("SELECT {SPRINT_COLS} FROM sprints WHERE id = $1"),
    )
    .bind(sprint_id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

/// スプリント + タスク取得
pub async fn get_sprint_with_tasks(pool: &PgPool, sprint_id: Uuid) -> Result<SprintWithTasks, AppError> {
    let sprint = get_sprint(pool, sprint_id).await?;
    let tasks = get_sprint_tasks(pool, sprint_id).await?;
    Ok(SprintWithTasks { sprint, tasks })
}

/// スプリントのタスク一覧
pub async fn get_sprint_tasks(pool: &PgPool, sprint_id: Uuid) -> Result<Vec<Task>, AppError> {
    Ok(sqlx::query_as::<_, Task>(
        &format!(
            "SELECT {TASK_COLS} FROM tasks WHERE sprint_id = $1 ORDER BY execution_order, created_at"
        ),
    )
    .bind(sprint_id)
    .fetch_all(pool)
    .await?)
}

/// プロジェクトのスプリント一覧
pub async fn list_sprints(pool: &PgPool, project_id: Uuid) -> Result<Vec<Sprint>, AppError> {
    Ok(sqlx::query_as::<_, Sprint>(
        &format!(
            "SELECT {SPRINT_COLS} FROM sprints WHERE project_id = $1 ORDER BY created_at DESC LIMIT 20"
        ),
    )
    .bind(project_id)
    .fetch_all(pool)
    .await?)
}

/// プロジェクトのアクティブスプリント取得
pub async fn get_active_sprint(pool: &PgPool, project_id: Uuid) -> Result<Option<Sprint>, AppError> {
    Ok(sqlx::query_as::<_, Sprint>(
        &format!(
            "SELECT {SPRINT_COLS} FROM sprints WHERE project_id = $1 AND status NOT IN ('completed', 'failed') \
             ORDER BY created_at DESC LIMIT 1"
        ),
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?)
}

/// スキャン完了時の更新
pub async fn update_scan_completed(
    pool: &PgPool,
    sprint_id: Uuid,
    analysis: &str,
    priority_actions: &serde_json::Value,
) -> Result<Sprint, AppError> {
    sqlx::query_as::<_, Sprint>(
        &format!(
            "UPDATE sprints SET scan_analysis = $2, priority_actions = $3 \
             WHERE id = $1 RETURNING {SPRINT_COLS}"
        ),
    )
    .bind(sprint_id)
    .bind(analysis)
    .bind(priority_actions)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// スプリントステータス更新
pub async fn update_status(
    pool: &PgPool,
    sprint_id: Uuid,
    status: &str,
) -> Result<Sprint, AppError> {
    sqlx::query_as::<_, Sprint>(
        &format!(
            "UPDATE sprints SET status = $2 WHERE id = $1 RETURNING {SPRINT_COLS}"
        ),
    )
    .bind(sprint_id)
    .bind(status)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// タスク選定 (採用/却下)
pub async fn select_tasks(
    pool: &PgPool,
    sprint_id: Uuid,
    approved_ids: &[Uuid],
    rejected_ids: &[Uuid],
) -> Result<Vec<Task>, AppError> {
    // 承認タスクを approved に
    for id in approved_ids {
        sqlx::query(
            "UPDATE tasks SET status = 'approved' WHERE id = $1 AND sprint_id = $2 AND status = 'proposed'",
        )
        .bind(id)
        .bind(sprint_id)
        .execute(pool)
        .await?;
    }

    // 却下タスクを cancelled に
    for id in rejected_ids {
        sqlx::query(
            "UPDATE tasks SET status = 'cancelled' WHERE id = $1 AND sprint_id = $2 AND status = 'proposed'",
        )
        .bind(id)
        .bind(sprint_id)
        .execute(pool)
        .await?;
    }

    get_sprint_tasks(pool, sprint_id).await
}

/// スプリント内の全承認済みタスクがヒアリング完了 (awaiting_approval) か確認
pub async fn all_tasks_ready(pool: &PgPool, sprint_id: Uuid) -> Result<bool, AppError> {
    // 承認済みタスク (cancelled 以外) のうち awaiting_approval でないものがあれば false
    let not_ready: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks \
         WHERE sprint_id = $1 \
         AND status NOT IN ('cancelled', 'awaiting_approval', 'completed', 'failed') \
         AND status != 'proposed'",
    )
    .bind(sprint_id)
    .fetch_one(pool)
    .await?;

    let total_approved: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE sprint_id = $1 AND status = 'awaiting_approval'",
    )
    .bind(sprint_id)
    .fetch_one(pool)
    .await?;

    Ok(not_ready == 0 && total_approved > 0)
}

/// 実行計画を保存し executing ステータスへ
pub async fn approve_plan(
    pool: &PgPool,
    sprint_id: Uuid,
    execution_plan: &str,
    max_parallel_tasks: i32,
) -> Result<Sprint, AppError> {
    sqlx::query_as::<_, Sprint>(
        &format!(
            "UPDATE sprints SET status = 'executing', execution_plan = $2, \
             max_parallel_tasks = $3, started_at = NOW() \
             WHERE id = $1 RETURNING {SPRINT_COLS}"
        ),
    )
    .bind(sprint_id)
    .bind(execution_plan)
    .bind(max_parallel_tasks)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// 振り返りデータを保存
pub async fn save_retrospective(
    pool: &PgPool,
    sprint_id: Uuid,
    retrospective: &str,
    improvement_suggestions: Option<&serde_json::Value>,
) -> Result<Sprint, AppError> {
    sqlx::query_as::<_, Sprint>(
        &format!(
            "UPDATE sprints SET status = 'retrospective', retrospective = $2, \
             improvement_suggestions = $3 \
             WHERE id = $1 RETURNING {SPRINT_COLS}"
        ),
    )
    .bind(sprint_id)
    .bind(retrospective)
    .bind(improvement_suggestions)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// ユーザーフィードバックを保存してスプリント完了
pub async fn complete_with_feedback(
    pool: &PgPool,
    sprint_id: Uuid,
    feedback: &str,
) -> Result<Sprint, AppError> {
    sqlx::query_as::<_, Sprint>(
        &format!(
            "UPDATE sprints SET status = 'completed', user_feedback = $2, completed_at = NOW() \
             WHERE id = $1 RETURNING {SPRINT_COLS}"
        ),
    )
    .bind(sprint_id)
    .bind(feedback)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// スプリント失敗
pub async fn fail_sprint(
    pool: &PgPool,
    sprint_id: Uuid,
    error_log: &str,
) -> Result<Sprint, AppError> {
    sqlx::query_as::<_, Sprint>(
        &format!(
            "UPDATE sprints SET status = 'failed', error_log = $2, completed_at = NOW() \
             WHERE id = $1 RETURNING {SPRINT_COLS}"
        ),
    )
    .bind(sprint_id)
    .bind(error_log)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// スプリント内の全タスクが終了状態か確認
/// (completed, failed, cancelled のみ)
pub async fn all_tasks_terminal(pool: &PgPool, sprint_id: Uuid) -> Result<bool, AppError> {
    let non_terminal: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks \
         WHERE sprint_id = $1 \
         AND status NOT IN ('completed', 'failed', 'cancelled')",
    )
    .bind(sprint_id)
    .fetch_one(pool)
    .await?;

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM tasks WHERE sprint_id = $1",
    )
    .bind(sprint_id)
    .fetch_one(pool)
    .await?;

    Ok(non_terminal == 0 && total > 0)
}

/// 前回のスプリント取得 (振り返り用)
pub async fn get_last_completed_sprint(
    pool: &PgPool,
    project_id: Uuid,
) -> Result<Option<SprintWithTasks>, AppError> {
    let sprint = sqlx::query_as::<_, Sprint>(
        &format!(
            "SELECT {SPRINT_COLS} FROM sprints \
             WHERE project_id = $1 AND status = 'completed' \
             ORDER BY completed_at DESC LIMIT 1"
        ),
    )
    .bind(project_id)
    .fetch_optional(pool)
    .await?;

    match sprint {
        Some(s) => {
            let tasks = get_sprint_tasks(pool, s.id).await?;
            Ok(Some(SprintWithTasks { sprint: s, tasks }))
        }
        None => Ok(None),
    }
}
