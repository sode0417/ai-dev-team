use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum SprintStatus {
    Selecting,
    Hearing,
    Planning,
    Executing,
    Retrospective,
    Improving,
    Completed,
    Failed,
}

impl SprintStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Selecting => "selecting",
            Self::Hearing => "hearing",
            Self::Planning => "planning",
            Self::Executing => "executing",
            Self::Retrospective => "retrospective",
            Self::Improving => "improving",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }

    pub fn from_str(s: &str) -> Self {
        match s {
            "selecting" => Self::Selecting,
            "hearing" => Self::Hearing,
            "planning" => Self::Planning,
            "executing" => Self::Executing,
            "retrospective" => Self::Retrospective,
            "improving" => Self::Improving,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            _ => Self::Selecting,
        }
    }
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Sprint {
    pub id: Uuid,
    pub project_id: Uuid,
    pub status: String,
    pub scan_analysis: Option<String>,
    pub priority_actions: Option<Value>,
    pub execution_plan: Option<String>,
    pub retrospective: Option<String>,
    pub improvement_suggestions: Option<Value>,
    pub user_feedback: Option<String>,
    pub improvement_results: Option<Value>,
    pub max_parallel_tasks: i32,
    pub error_log: Option<String>,
    pub created_at: DateTime<Utc>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// スプリント + 紐づくタスク一覧
#[derive(Debug, Serialize)]
pub struct SprintWithTasks {
    #[serde(flatten)]
    pub sprint: Sprint,
    pub tasks: Vec<super::super::tasks::model::Task>,
}

/// 計画承認リクエスト
#[derive(Debug, Deserialize)]
pub struct ApprovePlanRequest {
    pub max_parallel_tasks: Option<i32>,
}

/// ユーザーからの振り返りフィードバック
#[derive(Debug, Deserialize)]
pub struct SprintFeedbackRequest {
    pub feedback: String,
}

/// 改善適用結果
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImprovementResult {
    pub target: String,
    pub description: String,
    pub status: String,
    pub pr_url: Option<String>,
    pub issue_url: Option<String>,
    pub error: Option<String>,
}

/// タスク選定リクエスト (採用/却下)
#[derive(Debug, Deserialize)]
pub struct SelectTasksRequest {
    pub approved_task_ids: Vec<Uuid>,
    pub rejected_task_ids: Vec<Uuid>,
}
