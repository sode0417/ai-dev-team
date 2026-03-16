use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct ScanSession {
    pub id: Uuid,
    pub project_id: Uuid,
    pub status: String,
    pub analysis: Option<String>,
    pub priority_actions: Option<Value>,
    pub retrospective: Option<String>,
    pub improvement_suggestions: Option<Value>,
    pub error_log: Option<String>,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Claude CLI が返す JSON のパース用
#[derive(Debug, Deserialize)]
pub struct ScanAnalysisOutput {
    pub summary: String,
    pub retrospective: Option<String>,
    pub priority_actions: Vec<String>,
    pub task_proposals: Vec<TaskProposal>,
    #[serde(default)]
    pub improvement_suggestions: Vec<ImprovementSuggestion>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TaskProposal {
    pub repository_name: Option<String>,
    pub title: String,
    pub description: String,
    pub priority: Option<String>,
    pub proposal_type: Option<String>,
    pub issue_number: Option<i64>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImprovementSuggestion {
    pub target: String,
    pub description: String,
    pub reason: String,
}

/// スキャン結果 + 生成されたタスク一覧
#[derive(Debug, Serialize)]
pub struct ScanResult {
    #[serde(flatten)]
    pub session: ScanSession,
    pub tasks: Vec<super::super::tasks::model::Task>,
}
