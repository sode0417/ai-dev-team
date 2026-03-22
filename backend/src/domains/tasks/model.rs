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
    Hearing,
    Planning,
    AwaitingApproval,
    Executing,
    Reviewing,
    PendingCompletion,
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
    pub execution_group: i32,
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
    pub scan_id: Option<Uuid>,
    pub proposal_type: String,
    pub sprint_id: Option<Uuid>,
    pub issue_number: Option<i32>,
    pub issue_url: Option<String>,
    pub merge_status: Option<String>,
    pub merge_attempted_at: Option<DateTime<Utc>>,
    pub revision_count: i32,
    pub definition_of_done: Option<String>,
    pub cancel_reason: Option<String>,
    pub completion_note: Option<String>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct MergeLog {
    pub id: Uuid,
    pub task_id: Uuid,
    pub action: String,
    pub success: bool,
    pub message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskRequest {
    pub project_id: Uuid,
    pub repository_id: Option<Uuid>,
    pub title: String,
    pub description: String,
    pub priority: Option<TaskPriority>,
    pub definition_of_done: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTaskRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub priority: Option<TaskPriority>,
    pub definition_of_done: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ListTasksQuery {
    pub project_id: Option<Uuid>,
    pub status: Option<String>,
}

// ヒアリング関連

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TaskHearing {
    pub id: Uuid,
    pub task_id: Uuid,
    pub session_id: Option<Uuid>,
    pub phase: String,
    pub round: i32,
    pub questions: Value,
    pub answers: Option<Value>,
    pub status: String,
    pub created_at: DateTime<Utc>,
    pub answered_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteTaskRequest {
    pub skip_hearing: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct AnswerHearingRequest {
    pub answers: Vec<HearingAnswer>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HearingAnswer {
    pub index: i32,
    pub answer: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HearingQuestion {
    pub index: i32,
    pub question: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ExecuteIssueRequest {
    pub project_id: Uuid,
    pub repository_id: Uuid,
    pub issue_number: i32,
    pub issue_url: String,
    pub skip_hearing: Option<bool>,
    pub definition_of_done: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RejectPlanRequest {
    pub action: String,       // "replan" | "cancel"
    pub feedback: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct RequestRevisionRequest {
    pub instructions: String,
}

#[derive(Debug, Deserialize)]
pub struct CancelTaskRequest {
    pub reason: Option<String>,
}
