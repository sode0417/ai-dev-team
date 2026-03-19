use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExecutionSession {
    pub id: Uuid,
    pub task_id: Uuid,
    pub attempt: i32,
    pub status: String,
    pub worktree_path: Option<String>,
    pub branch_name: Option<String>,
    pub plan_output: Option<String>,
    pub review_output: Option<String>,
    pub review_verdict: Option<String>,
    pub test_output: Option<String>,
    pub test_passed: Option<bool>,
    pub qa_output: Option<String>,
    pub qa_passed: Option<bool>,
    pub qa_screenshots: Option<Value>,
    pub revision_instructions: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ExecutionLog {
    pub id: Uuid,
    pub session_id: Uuid,
    pub phase: String,
    pub iteration: i32,
    pub level: String,
    pub message: String,
    pub metadata: Option<Value>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListLogsQuery {
    pub phase: Option<String>,
    pub level: Option<String>,
}
