export type TaskStatus =
  | "proposed"
  | "approved"
  | "queued"
  | "planning"
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

export interface ScanResult extends ScanSession {
  tasks: Task[];
}

export interface ScanWsMessage {
  scan_id: string;
  phase: string;
  message: string;
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
