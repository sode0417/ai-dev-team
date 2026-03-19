use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use super::model::*;

pub async fn list_sessions(pool: &PgPool, task_id: Uuid) -> Result<Vec<ExecutionSession>, AppError> {
    Ok(sqlx::query_as::<_, ExecutionSession>(
        "SELECT id, task_id, attempt, status, worktree_path, branch_name, \
         plan_output, review_output, review_verdict, test_output, test_passed, \
         qa_output, qa_passed, qa_screenshots, \
         revision_instructions, started_at, completed_at \
         FROM execution_sessions WHERE task_id = $1 ORDER BY attempt DESC",
    )
    .bind(task_id)
    .fetch_all(pool)
    .await?)
}

pub async fn create_session(
    pool: &PgPool,
    task_id: Uuid,
    worktree_path: Option<&str>,
    branch_name: Option<&str>,
) -> Result<ExecutionSession, AppError> {
    create_session_with_instructions(pool, task_id, worktree_path, branch_name, None).await
}

pub async fn create_session_with_instructions(
    pool: &PgPool,
    task_id: Uuid,
    worktree_path: Option<&str>,
    branch_name: Option<&str>,
    revision_instructions: Option<&str>,
) -> Result<ExecutionSession, AppError> {
    let attempt: i32 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(attempt), 0) + 1 FROM execution_sessions WHERE task_id = $1",
    )
    .bind(task_id)
    .fetch_one(pool)
    .await?;

    Ok(sqlx::query_as::<_, ExecutionSession>(
        "INSERT INTO execution_sessions (task_id, attempt, worktree_path, branch_name, revision_instructions) \
         VALUES ($1, $2, $3, $4, $5) \
         RETURNING id, task_id, attempt, status, worktree_path, branch_name, \
         plan_output, review_output, review_verdict, test_output, test_passed, \
         qa_output, qa_passed, qa_screenshots, \
         revision_instructions, started_at, completed_at",
    )
    .bind(task_id)
    .bind(attempt)
    .bind(worktree_path)
    .bind(branch_name)
    .bind(revision_instructions)
    .fetch_one(pool)
    .await?)
}

pub async fn update_session(
    pool: &PgPool,
    session_id: Uuid,
    status: &str,
    plan_output: Option<&str>,
    review_output: Option<&str>,
    review_verdict: Option<&str>,
    test_output: Option<&str>,
    test_passed: Option<bool>,
) -> Result<ExecutionSession, AppError> {
    update_session_full(pool, session_id, status, plan_output, review_output, review_verdict, test_output, test_passed, None, None, None).await
}

pub async fn update_session_with_qa(
    pool: &PgPool,
    session_id: Uuid,
    status: &str,
    qa_output: Option<&str>,
    qa_passed: Option<bool>,
    qa_screenshots: Option<&Value>,
) -> Result<ExecutionSession, AppError> {
    update_session_full(pool, session_id, status, None, None, None, None, None, qa_output, qa_passed, qa_screenshots).await
}

async fn update_session_full(
    pool: &PgPool,
    session_id: Uuid,
    status: &str,
    plan_output: Option<&str>,
    review_output: Option<&str>,
    review_verdict: Option<&str>,
    test_output: Option<&str>,
    test_passed: Option<bool>,
    qa_output: Option<&str>,
    qa_passed: Option<bool>,
    qa_screenshots: Option<&Value>,
) -> Result<ExecutionSession, AppError> {
    let completed_at = if status == "completed" || status == "failed" {
        Some(chrono::Utc::now())
    } else {
        None
    };

    Ok(sqlx::query_as::<_, ExecutionSession>(
        r#"UPDATE execution_sessions SET
            status = $2,
            plan_output = COALESCE($3, plan_output),
            review_output = COALESCE($4, review_output),
            review_verdict = COALESCE($5, review_verdict),
            test_output = COALESCE($6, test_output),
            test_passed = COALESCE($7, test_passed),
            completed_at = COALESCE($8, completed_at),
            qa_output = COALESCE($9, qa_output),
            qa_passed = COALESCE($10, qa_passed),
            qa_screenshots = COALESCE($11, qa_screenshots)
        WHERE id = $1
        RETURNING id, task_id, attempt, status, worktree_path, branch_name,
        plan_output, review_output, review_verdict, test_output, test_passed,
        qa_output, qa_passed, qa_screenshots,
        revision_instructions, started_at, completed_at"#,
    )
    .bind(session_id)
    .bind(status)
    .bind(plan_output)
    .bind(review_output)
    .bind(review_verdict)
    .bind(test_output)
    .bind(test_passed)
    .bind(completed_at)
    .bind(qa_output)
    .bind(qa_passed)
    .bind(qa_screenshots)
    .fetch_one(pool)
    .await?)
}

/// worktree_path を NULL にクリア（ワークツリー削除後に呼び出す）
pub async fn clear_worktree_path(pool: &PgPool, session_id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE execution_sessions SET worktree_path = NULL WHERE id = $1")
        .bind(session_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_logs(
    pool: &PgPool,
    session_id: Uuid,
    query: &ListLogsQuery,
) -> Result<Vec<ExecutionLog>, AppError> {
    match (query.phase.as_deref(), query.level.as_deref()) {
        (Some(phase), Some(level)) => {
            Ok(sqlx::query_as::<_, ExecutionLog>(
                "SELECT id, session_id, phase, iteration, level, message, metadata, created_at \
                 FROM execution_logs WHERE session_id = $1 AND phase = $2 AND level = $3 \
                 ORDER BY created_at",
            )
            .bind(session_id)
            .bind(phase)
            .bind(level)
            .fetch_all(pool)
            .await?)
        }
        (Some(phase), None) => {
            Ok(sqlx::query_as::<_, ExecutionLog>(
                "SELECT id, session_id, phase, iteration, level, message, metadata, created_at \
                 FROM execution_logs WHERE session_id = $1 AND phase = $2 ORDER BY created_at",
            )
            .bind(session_id)
            .bind(phase)
            .fetch_all(pool)
            .await?)
        }
        (None, Some(level)) => {
            Ok(sqlx::query_as::<_, ExecutionLog>(
                "SELECT id, session_id, phase, iteration, level, message, metadata, created_at \
                 FROM execution_logs WHERE session_id = $1 AND level = $2 ORDER BY created_at",
            )
            .bind(session_id)
            .bind(level)
            .fetch_all(pool)
            .await?)
        }
        (None, None) => {
            Ok(sqlx::query_as::<_, ExecutionLog>(
                "SELECT id, session_id, phase, iteration, level, message, metadata, created_at \
                 FROM execution_logs WHERE session_id = $1 ORDER BY created_at",
            )
            .bind(session_id)
            .fetch_all(pool)
            .await?)
        }
    }
}

pub async fn add_log(
    pool: &PgPool,
    session_id: Uuid,
    phase: &str,
    iteration: i32,
    level: &str,
    message: &str,
    metadata: Option<&Value>,
) -> Result<ExecutionLog, AppError> {
    Ok(sqlx::query_as::<_, ExecutionLog>(
        "INSERT INTO execution_logs (session_id, phase, iteration, level, message, metadata) \
         VALUES ($1, $2, $3, $4, $5, $6) \
         RETURNING id, session_id, phase, iteration, level, message, metadata, created_at",
    )
    .bind(session_id)
    .bind(phase)
    .bind(iteration)
    .bind(level)
    .bind(message)
    .bind(metadata)
    .fetch_one(pool)
    .await?)
}
