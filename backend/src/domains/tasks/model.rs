use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type, PartialEq)]
#[sqlx(type_name = "task_status", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Proposed,
    Approved,
    Queued,
    Planning,
    Executing,
    Reviewing,
    Completed,
    Failed,
    Cancelled,
    Blocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "task_priority", rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TaskPriority {
    Critical,
    High,
    Medium,
    Low,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub project_id: Uuid,
    pub repository_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: TaskPriority,
    pub depends_on: Option<Uuid>,
    pub execution_order: i32,
    pub proposed_by: String,
    pub plan: Option<String>,
    pub pr_url: Option<String>,
    pub changed_files: Option<Value>,
    pub diff_stats: Option<String>,
    pub retry_count: i32,
    pub max_retries: i32,
    pub error_log: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub project_id: Uuid,
    pub repository_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub priority: Option<TaskPriority>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<TaskPriority>,
}

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub project_id: Option<Uuid>,
    pub status: Option<String>,
}
