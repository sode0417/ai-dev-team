use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use super::model::*;

pub async fn list_tasks(pool: &PgPool, query: &ListTasksQuery) -> Result<Vec<Task>, AppError> {
    let mut sql = String::from(
        "SELECT id, project_id, repository_id, title, description, status, priority, \
         depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at \
         FROM tasks WHERE 1=1",
    );
    let mut binds: Vec<String> = vec![];

    if let Some(ref project_id) = query.project_id {
        binds.push(project_id.to_string());
        sql.push_str(&format!(" AND project_id = ${}", binds.len()));
    }
    if let Some(ref status) = query.status {
        binds.push(status.clone());
        sql.push_str(&format!(" AND status::text = ${}", binds.len()));
    }

    sql.push_str(" ORDER BY execution_order, created_at DESC");

    // sqlx の動的クエリには query_builder を使うが、シンプルにするため分岐
    match (query.project_id, query.status.as_deref()) {
        (Some(pid), Some(status)) => {
            Ok(sqlx::query_as::<_, Task>(
                "SELECT id, project_id, repository_id, title, description, status, priority, \
                 depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
                 retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at \
                 FROM tasks WHERE project_id = $1 AND status::text = $2 \
                 ORDER BY execution_order, created_at DESC",
            )
            .bind(pid)
            .bind(status)
            .fetch_all(pool)
            .await?)
        }
        (Some(pid), None) => {
            Ok(sqlx::query_as::<_, Task>(
                "SELECT id, project_id, repository_id, title, description, status, priority, \
                 depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
                 retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at \
                 FROM tasks WHERE project_id = $1 \
                 ORDER BY execution_order, created_at DESC",
            )
            .bind(pid)
            .fetch_all(pool)
            .await?)
        }
        (None, Some(status)) => {
            Ok(sqlx::query_as::<_, Task>(
                "SELECT id, project_id, repository_id, title, description, status, priority, \
                 depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
                 retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at \
                 FROM tasks WHERE status::text = $1 \
                 ORDER BY execution_order, created_at DESC",
            )
            .bind(status)
            .fetch_all(pool)
            .await?)
        }
        (None, None) => {
            Ok(sqlx::query_as::<_, Task>(
                "SELECT id, project_id, repository_id, title, description, status, priority, \
                 depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
                 retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at \
                 FROM tasks ORDER BY execution_order, created_at DESC",
            )
            .fetch_all(pool)
            .await?)
        }
    }
}

pub async fn get_task(pool: &PgPool, id: Uuid) -> Result<Task, AppError> {
    sqlx::query_as::<_, Task>(
        "SELECT id, project_id, repository_id, title, description, status, priority, \
         depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at \
         FROM tasks WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn create_task(pool: &PgPool, req: &CreateTaskRequest) -> Result<Task, AppError> {
    // プロジェクト存在確認
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM projects WHERE id = $1)")
        .bind(req.project_id)
        .fetch_one(pool)
        .await?;
    if !exists {
        return Err(AppError::Validation("Project not found".to_string()));
    }

    let priority = req.priority.clone().unwrap_or(TaskPriority::Medium);

    sqlx::query_as::<_, Task>(
        "INSERT INTO tasks (project_id, repository_id, title, description, priority) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, project_id, repository_id, title, description, status, priority, \
         depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at",
    )
    .bind(req.project_id)
    .bind(req.repository_id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&priority)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn create_task_from_issue(
    pool: &PgPool,
    project_id: Uuid,
    repository_id: Uuid,
    issue_number: i32,
    issue_url: &str,
    title: &str,
    description: &str,
) -> Result<Task, AppError> {
    sqlx::query_as::<_, Task>(
        "INSERT INTO tasks (project_id, repository_id, title, description, issue_number, issue_url, proposal_type) \
         VALUES ($1, $2, $3, $4, $5, $6, 'development') \
         RETURNING id, project_id, repository_id, title, description, status, priority, \
         depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at",
    )
    .bind(project_id)
    .bind(repository_id)
    .bind(title)
    .bind(description)
    .bind(issue_number)
    .bind(issue_url)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_task(pool: &PgPool, id: Uuid, req: &UpdateTaskRequest) -> Result<Task, AppError> {
    sqlx::query_as::<_, Task>(
        r#"UPDATE tasks SET
            title = COALESCE($2, title),
            description = COALESCE($3, description),
            priority = COALESCE($4, priority),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, repository_id, title, description, status, priority,
        depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats,
        retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at"#,
    )
    .bind(id)
    .bind(&req.title)
    .bind(&req.description)
    .bind(&req.priority)
    .fetch_optional(pool)
    .await?
    .ok_or(AppError::NotFound)
}

pub async fn approve_task(pool: &PgPool, id: Uuid) -> Result<Task, AppError> {
    let task = get_task(pool, id).await?;
    if task.status != TaskStatus::Proposed {
        return Err(AppError::Validation(format!(
            "Task status must be 'proposed' to approve, got '{:?}'",
            task.status
        )));
    }

    sqlx::query_as::<_, Task>(
        r#"UPDATE tasks SET status = 'approved', updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, repository_id, title, description, status, priority,
        depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats,
        retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

pub async fn cancel_task(pool: &PgPool, id: Uuid) -> Result<Task, AppError> {
    let task = get_task(pool, id).await?;
    if task.status == TaskStatus::Completed || task.status == TaskStatus::Cancelled {
        return Err(AppError::Validation("Task is already completed or cancelled".to_string()));
    }

    sqlx::query_as::<_, Task>(
        r#"UPDATE tasks SET status = 'cancelled', updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, repository_id, title, description, status, priority,
        depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats,
        retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// タスクのステータスと実行関連フィールドを更新（executor から使用）
pub async fn update_task_execution(
    pool: &PgPool,
    id: Uuid,
    status: TaskStatus,
    plan: Option<&str>,
    pr_url: Option<&str>,
    changed_files: Option<&serde_json::Value>,
    diff_stats: Option<&str>,
    error_log: Option<&str>,
) -> Result<Task, AppError> {
    let now = chrono::Utc::now();
    let started_at = if status == TaskStatus::Planning {
        Some(now)
    } else {
        None
    };
    let completed_at = if status == TaskStatus::Completed || status == TaskStatus::Failed {
        Some(now)
    } else {
        None
    };

    sqlx::query_as::<_, Task>(
        r#"UPDATE tasks SET
            status = $2,
            plan = COALESCE($3, plan),
            pr_url = COALESCE($4, pr_url),
            changed_files = COALESCE($5, changed_files),
            diff_stats = COALESCE($6, diff_stats),
            error_log = COALESCE($7, error_log),
            started_at = COALESCE($8, started_at),
            completed_at = COALESCE($9, completed_at),
            updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, repository_id, title, description, status, priority,
        depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats,
        retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at"#,
    )
    .bind(id)
    .bind(&status)
    .bind(plan)
    .bind(pr_url)
    .bind(changed_files)
    .bind(diff_stats)
    .bind(error_log)
    .bind(started_at)
    .bind(completed_at)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

// === ヒアリング CRUD ===

pub async fn create_hearing(
    pool: &PgPool,
    task_id: Uuid,
    session_id: Option<Uuid>,
    phase: &str,
    round: i32,
    questions: &Value,
) -> Result<TaskHearing, AppError> {
    Ok(sqlx::query_as::<_, TaskHearing>(
        "INSERT INTO task_hearings (task_id, session_id, phase, round, questions) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, task_id, session_id, phase, round, questions, answers, status, created_at, answered_at",
    )
    .bind(task_id)
    .bind(session_id)
    .bind(phase)
    .bind(round)
    .bind(questions)
    .fetch_one(pool)
    .await?)
}

pub async fn answer_hearing(
    pool: &PgPool,
    hearing_id: Uuid,
    answers: &Value,
) -> Result<TaskHearing, AppError> {
    Ok(sqlx::query_as::<_, TaskHearing>(
        r#"UPDATE task_hearings SET
            answers = $2,
            status = 'answered',
            answered_at = NOW()
        WHERE id = $1
        RETURNING id, task_id, session_id, phase, round, questions, answers, status, created_at, answered_at"#,
    )
    .bind(hearing_id)
    .bind(answers)
    .fetch_one(pool)
    .await?)
}

pub async fn get_latest_hearing(pool: &PgPool, task_id: Uuid) -> Result<Option<TaskHearing>, AppError> {
    Ok(sqlx::query_as::<_, TaskHearing>(
        "SELECT id, task_id, session_id, phase, round, questions, answers, status, created_at, answered_at \
         FROM task_hearings WHERE task_id = $1 ORDER BY created_at DESC LIMIT 1",
    )
    .bind(task_id)
    .fetch_optional(pool)
    .await?)
}

pub async fn list_hearings(pool: &PgPool, task_id: Uuid) -> Result<Vec<TaskHearing>, AppError> {
    Ok(sqlx::query_as::<_, TaskHearing>(
        "SELECT id, task_id, session_id, phase, round, questions, answers, status, created_at, answered_at \
         FROM task_hearings WHERE task_id = $1 ORDER BY created_at",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?)
}

/// 計画承認: awaiting_approval → executing
pub async fn approve_plan(pool: &PgPool, id: Uuid) -> Result<Task, AppError> {
    let task = get_task(pool, id).await?;
    if task.status != TaskStatus::AwaitingApproval {
        return Err(AppError::Validation(format!(
            "Task must be 'awaiting_approval' to approve plan, got '{:?}'",
            task.status
        )));
    }

    sqlx::query_as::<_, Task>(
        r#"UPDATE tasks SET status = 'executing', updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, repository_id, title, description, status, priority,
        depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats,
        retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at"#,
    )
    .bind(id)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

/// 計画却下: replan → hearing に戻す, cancel → cancelled
pub async fn reject_plan(pool: &PgPool, id: Uuid, action: &str) -> Result<Task, AppError> {
    let task = get_task(pool, id).await?;
    if task.status != TaskStatus::AwaitingApproval {
        return Err(AppError::Validation(format!(
            "Task must be 'awaiting_approval' to reject plan, got '{:?}'",
            task.status
        )));
    }

    let new_status = if action == "cancel" { "cancelled" } else { "hearing" };

    sqlx::query_as::<_, Task>(
        r#"UPDATE tasks SET status = $2::task_status, plan = CASE WHEN $2 = 'hearing' THEN NULL ELSE plan END, updated_at = NOW()
        WHERE id = $1
        RETURNING id, project_id, repository_id, title, description, status, priority,
        depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats,
        retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at"#,
    )
    .bind(id)
    .bind(new_status)
    .fetch_one(pool)
    .await
    .map_err(AppError::from)
}

// === マージ関連 ===

/// マージ対象タスクを取得（pr_url あり + completed + merge_status = 'pending'）
pub async fn list_mergeable_tasks(pool: &PgPool) -> Result<Vec<Task>, AppError> {
    Ok(sqlx::query_as::<_, Task>(
        "SELECT id, project_id, repository_id, title, description, status, priority, \
         depends_on, execution_order, execution_group, proposed_by, plan, pr_url, changed_files, diff_stats, \
         retry_count, max_retries, error_log, created_at, started_at, completed_at, updated_at, \
         scan_id, proposal_type, sprint_id, issue_number, issue_url, merge_status, merge_attempted_at \
         FROM tasks WHERE pr_url IS NOT NULL AND status = 'completed' AND merge_status = 'pending' \
         ORDER BY completed_at",
    )
    .fetch_all(pool)
    .await?)
}

/// マージ状態を更新
pub async fn update_merge_status(
    pool: &PgPool,
    id: Uuid,
    status: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE tasks SET merge_status = $2, merge_attempted_at = NOW(), updated_at = NOW() WHERE id = $1",
    )
    .bind(id)
    .bind(status)
    .execute(pool)
    .await?;
    Ok(())
}

/// マージログを追加
pub async fn add_merge_log(
    pool: &PgPool,
    task_id: Uuid,
    action: &str,
    success: bool,
    message: Option<&str>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO merge_logs (task_id, action, success, message) VALUES ($1, $2, $3, $4)",
    )
    .bind(task_id)
    .bind(action)
    .bind(success)
    .bind(message)
    .execute(pool)
    .await?;
    Ok(())
}

/// ヒアリング全回答を文字列で取得（プロンプト挿入用）
pub async fn get_hearing_context(pool: &PgPool, task_id: Uuid) -> Result<String, AppError> {
    let hearings = list_hearings(pool, task_id).await?;
    let mut context = String::new();
    for h in hearings {
        if h.status != "answered" {
            continue;
        }
        let questions: Vec<HearingQuestion> = serde_json::from_value(h.questions).unwrap_or_default();
        let answers: Vec<HearingAnswer> = h.answers.map(|a| serde_json::from_value(a).unwrap_or_default()).unwrap_or_default();
        for q in &questions {
            if let Some(a) = answers.iter().find(|a| a.index == q.index) {
                context.push_str(&format!("Q: {}\nA: {}\n\n", q.question, a.answer));
            }
        }
    }
    Ok(context)
}
