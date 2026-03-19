export type TaskStatus =
  | "proposed"
  | "approved"
  | "queued"
  | "hearing"
  | "planning"
  | "awaiting_approval"
  | "executing"
  | "reviewing"
  | "completed"
  | "failed"
  | "cancelled"
  | "blocked";

export type TaskPriority = "critical" | "high" | "medium" | "low";

export interface Project {
  id: string;
  name: string;
  description: string | null;
  created_at: string;
  updated_at: string;
  repositories: ProjectRepository[];
}

export interface ProjectRepository {
  id: string;
  project_id: string;
  owner: string;
  name: string;
  default_branch: string;
  local_path: string | null;
  created_at: string;
}

export interface Task {
  id: string;
  project_id: string;
  repository_id: string | null;
  title: string;
  description: string;
  status: TaskStatus;
  priority: TaskPriority;
  depends_on: string | null;
  execution_order: number;
  execution_group: number;
  proposed_by: string;
  plan: string | null;
  pr_url: string | null;
  changed_files: string[] | null;
  diff_stats: string | null;
  retry_count: number;
  max_retries: number;
  error_log: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
  updated_at: string;
  scan_id: string | null;
  proposal_type: string;
  sprint_id: string | null;
  issue_number: number | null;
  issue_url: string | null;
}

export interface ExecutionSession {
  id: string;
  task_id: string;
  attempt: number;
  status: string;
  worktree_path: string | null;
  branch_name: string | null;
  plan_output: string | null;
  review_output: string | null;
  review_verdict: string | null;
  test_output: string | null;
  test_passed: boolean | null;
  qa_output: string | null;
  qa_passed: boolean | null;
  qa_screenshots: string[] | null;
  started_at: string;
  completed_at: string | null;
}

export interface ExecutionLog {
  id: string;
  session_id: string;
  phase: string;
  iteration: number;
  level: string;
  message: string;
  metadata: Record<string, unknown> | null;
  created_at: string;
}

export interface DashboardData {
  total_projects: number;
  total_tasks: number;
  active_tasks: number;
  completed_tasks: number;
  failed_tasks: number;
  recent_tasks: Task[];
}

export interface WsMessage {
  task_id: string;
  phase: string;
  message: string;
}

export interface ScanSession {
  id: string;
  project_id: string;
  status: string;
  analysis: string | null;
  priority_actions: string[] | null;
  retrospective: string | null;
  improvement_suggestions: ImprovementSuggestion[] | null;
  error_log: string | null;
  started_at: string;
  completed_at: string | null;
}

export interface ImprovementSuggestion {
  target: string;
  description: string;
  reason: string;
}

export interface ImprovementResultItem {
  target: string;
  description: string;
  status: "applied" | "failed" | "skipped";
  pr_url: string | null;
  issue_url: string | null;
  error: string | null;
}

export interface ScanResult extends ScanSession {
  tasks: Task[];
}

export interface ScanWsMessage {
  scan_id: string;
  phase: string;
  message: string;
}

export type SprintStatus =
  | "selecting"
  | "hearing"
  | "planning"
  | "executing"
  | "retrospective"
  | "improving"
  | "completed"
  | "failed";

export interface Sprint {
  id: string;
  project_id: string;
  status: SprintStatus;
  scan_analysis: string | null;
  priority_actions: string[] | null;
  execution_plan: string | null;
  retrospective: string | null;
  improvement_suggestions: ImprovementSuggestion[] | null;
  user_feedback: string | null;
  improvement_results: ImprovementResultItem[] | null;
  max_parallel_tasks: number;
  error_log: string | null;
  created_at: string;
  started_at: string | null;
  completed_at: string | null;
}

export interface SprintWithTasks extends Sprint {
  tasks: Task[];
}

export interface SprintWsMessage {
  sprint_id: string;
  phase: string;
  message: string;
  parallel_tasks?: {
    task_id: string;
    title: string;
    status: TaskStatus;
  }[];
}

export interface HearingQuestion {
  index: number;
  question: string;
  options?: string[];
}

export interface HearingAnswer {
  index: number;
  answer: string;
}

export interface TaskHearing {
  id: string;
  task_id: string;
  session_id: string | null;
  phase: "pre_plan" | "in_plan";
  round: number;
  questions: HearingQuestion[];
  answers: HearingAnswer[] | null;
  status: "pending" | "answered" | "skipped";
  created_at: string;
  answered_at: string | null;
}

export interface GitHubLabel {
  name: string;
  color: string;
}

export interface GitHubUser {
  login: string;
  avatar_url: string;
}

export interface GitHubIssue {
  number: number;
  title: string;
  state: string;
  body: string | null;
  labels: GitHubLabel[];
  user: GitHubUser | null;
  html_url: string;
  created_at: string;
  updated_at: string;
  comments: number;
}

export interface GitHubPullRequest {
  number: number;
  title: string;
  state: string;
  draft: boolean | null;
  user: GitHubUser | null;
  html_url: string;
  created_at: string;
  updated_at: string;
  head: { ref: string };
  base: { ref: string };
}

// Auth
export interface AuthResponse {
  access_token: string;
  refresh_token: string;
  expires_in: number;
}

export interface LoginRequest {
  username: string;
  password: string;
}

export interface User {
  id: string;
  username: string;
}
