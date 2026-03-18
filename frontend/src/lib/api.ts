export { API_BASE } from "./config";

import { API_BASE } from "./config";

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const { getAccessToken, refreshAccessToken, clearTokens } = await import("./auth");

  const headers: Record<string, string> = {
    "Content-Type": "application/json",
    ...(options?.headers as Record<string, string>),
  };

  const token = getAccessToken();
  if (token) {
    headers["Authorization"] = `Bearer ${token}`;
  }

  let res = await fetch(`${API_BASE}${path}`, { ...options, headers });

  // 401 の場合、リフレッシュしてリトライ
  if (res.status === 401 && token) {
    const refreshed = await refreshAccessToken();
    if (refreshed) {
      const newToken = getAccessToken();
      if (newToken) {
        headers["Authorization"] = `Bearer ${newToken}`;
      }
      res = await fetch(`${API_BASE}${path}`, { ...options, headers });
    } else {
      clearTokens();
      if (typeof window !== "undefined") {
        window.location.href = "/login";
      }
      throw new Error("Session expired");
    }
  }

  if (!res.ok) {
    const body = await res.json().catch(() => ({}));
    throw new Error(body?.error?.message || `API error: ${res.status}`);
  }

  return res.json();
}

// Dashboard
export function fetchDashboard() {
  return request<{ data: import("@/types").DashboardData }>("/api/dashboard");
}

// Projects
export function fetchProjects() {
  return request<{ data: import("@/types").Project[] }>("/api/projects");
}

export function fetchProject(id: string) {
  return request<{ data: import("@/types").Project }>(`/api/projects/${id}`);
}

export function updateProject(id: string, body: { name?: string; description?: string }) {
  return request<{ data: import("@/types").Project }>(`/api/projects/${id}`, {
    method: "PUT",
    body: JSON.stringify(body),
  });
}

export function createProject(body: { name: string; description?: string }) {
  return request<{ data: import("@/types").Project }>("/api/projects", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function addRepository(
  projectId: string,
  body: { owner: string; name: string; default_branch?: string; local_path?: string }
) {
  return request<{ data: import("@/types").ProjectRepository }>(
    `/api/projects/${projectId}/repositories`,
    { method: "POST", body: JSON.stringify(body) }
  );
}

// Tasks
export function fetchTasks(params?: { project_id?: string; status?: string }) {
  const query = new URLSearchParams();
  if (params?.project_id) query.set("project_id", params.project_id);
  if (params?.status) query.set("status", params.status);
  const qs = query.toString();
  return request<{ data: import("@/types").Task[] }>(`/api/tasks${qs ? `?${qs}` : ""}`);
}

export function fetchTask(id: string) {
  return request<{ data: import("@/types").Task }>(`/api/tasks/${id}`);
}

export function createTask(body: {
  project_id: string;
  repository_id?: string;
  title: string;
  description: string;
  priority?: string;
}) {
  return request<{ data: import("@/types").Task }>("/api/tasks", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

export function approveTask(id: string) {
  return request<{ data: import("@/types").Task }>(`/api/tasks/${id}/approve`, {
    method: "POST",
  });
}

export function executeTask(id: string, skipHearing?: boolean) {
  return request<{ data: import("@/types").Task }>(`/api/tasks/${id}/execute`, {
    method: "POST",
    body: JSON.stringify({ skip_hearing: skipHearing ?? false }),
  });
}

export function cancelTask(id: string) {
  return request<{ data: import("@/types").Task }>(`/api/tasks/${id}/cancel`, {
    method: "POST",
  });
}

// Issue 単独実行
export function executeIssue(body: {
  project_id: string;
  repository_id: string;
  issue_number: number;
  issue_url: string;
  skip_hearing?: boolean;
}) {
  return request<{ data: import("@/types").Task }>("/api/tasks/execute-issue", {
    method: "POST",
    body: JSON.stringify(body),
  });
}

// Hearings
export function fetchHearings(taskId: string) {
  return request<{ data: import("@/types").TaskHearing[] }>(
    `/api/tasks/${taskId}/hearings`
  );
}

export function answerHearing(taskId: string, answers: import("@/types").HearingAnswer[]) {
  return request<{ data: import("@/types").Task; hearing: import("@/types").TaskHearing }>(
    `/api/tasks/${taskId}/hearing/answer`,
    { method: "POST", body: JSON.stringify({ answers }) }
  );
}

export function approvePlan(taskId: string) {
  return request<{ data: import("@/types").Task }>(
    `/api/tasks/${taskId}/approve-plan`,
    { method: "POST" }
  );
}

export function rejectPlan(taskId: string, action: "replan" | "cancel", feedback?: string) {
  return request<{ data: import("@/types").Task }>(
    `/api/tasks/${taskId}/reject-plan`,
    { method: "POST", body: JSON.stringify({ action, feedback }) }
  );
}

// Scans
export function scanProject(projectId: string) {
  return request<{ data: { scan_id: string } }>(`/api/projects/${projectId}/scan`, {
    method: "POST",
  });
}

export function fetchScans(projectId: string) {
  return request<{ data: import("@/types").ScanSession[] }>(
    `/api/projects/${projectId}/scans`
  );
}

export function fetchScanResult(scanId: string) {
  return request<{ data: import("@/types").ScanResult }>(
    `/api/scans/${scanId}`
  );
}

// Sprints
export function createSprint(projectId: string) {
  return request<{ data: { sprint_id: string } }>(`/api/projects/${projectId}/sprints`, {
    method: "POST",
  });
}

export function fetchSprints(projectId: string) {
  return request<{ data: import("@/types").Sprint[] }>(
    `/api/projects/${projectId}/sprints`
  );
}

export function fetchActiveSprint(projectId: string) {
  return request<{ data: import("@/types").SprintWithTasks | null }>(
    `/api/projects/${projectId}/sprint/active`
  );
}

export function fetchSprint(sprintId: string) {
  return request<{ data: import("@/types").SprintWithTasks }>(
    `/api/sprints/${sprintId}`
  );
}

export function selectSprintTasks(
  sprintId: string,
  approvedTaskIds: string[],
  rejectedTaskIds: string[]
) {
  return request<{ data: import("@/types").Task[] }>(
    `/api/sprints/${sprintId}/select-tasks`,
    {
      method: "POST",
      body: JSON.stringify({
        approved_task_ids: approvedTaskIds,
        rejected_task_ids: rejectedTaskIds,
      }),
    }
  );
}

export function startSprintHearing(sprintId: string) {
  return request<{ data: import("@/types").Sprint }>(
    `/api/sprints/${sprintId}/start-hearing`,
    { method: "POST" }
  );
}

export function fetchSprintReadiness(sprintId: string) {
  return request<{
    data: {
      all_ready: boolean;
      tasks: { id: string; title: string; status: string }[];
    };
  }>(`/api/sprints/${sprintId}/readiness`);
}

export function createSprintPlan(sprintId: string) {
  return request<{ data: import("@/types").Sprint }>(
    `/api/sprints/${sprintId}/plan`,
    { method: "POST" }
  );
}

export function approveSprintPlan(sprintId: string, maxParallelTasks?: number) {
  return request<{ data: import("@/types").Sprint }>(
    `/api/sprints/${sprintId}/approve-plan`,
    {
      method: "POST",
      body: JSON.stringify({ max_parallel_tasks: maxParallelTasks ?? 3 }),
    }
  );
}

export function cancelSprint(sprintId: string) {
  return request<{ data: import("@/types").Sprint }>(
    `/api/sprints/${sprintId}/cancel`,
    { method: "POST" }
  );
}

export function submitSprintFeedback(sprintId: string, feedback: string) {
  return request<{ data: import("@/types").Sprint }>(
    `/api/sprints/${sprintId}/feedback`,
    { method: "POST", body: JSON.stringify({ feedback }) }
  );
}

// Executions
export function fetchExecutions(taskId: string) {
  return request<{ data: import("@/types").ExecutionSession[] }>(
    `/api/tasks/${taskId}/executions`
  );
}

export function fetchExecutionLogs(sessionId: string) {
  return request<{ data: import("@/types").ExecutionLog[] }>(
    `/api/executions/${sessionId}/logs`
  );
}

// GitHub Issues / PRs
export function fetchRepositoryIssues(
  projectId: string,
  repoId: string,
  params?: { state?: string; page?: number; per_page?: number }
) {
  const query = new URLSearchParams();
  if (params?.state) query.set("state", params.state);
  if (params?.page) query.set("page", String(params.page));
  if (params?.per_page) query.set("per_page", String(params.per_page));
  const qs = query.toString();
  return request<{ data: import("@/types").GitHubIssue[] }>(
    `/api/projects/${projectId}/repositories/${repoId}/issues${qs ? `?${qs}` : ""}`
  );
}

export function fetchRepositoryPulls(
  projectId: string,
  repoId: string,
  params?: { state?: string; page?: number; per_page?: number }
) {
  const query = new URLSearchParams();
  if (params?.state) query.set("state", params.state);
  if (params?.page) query.set("page", String(params.page));
  if (params?.per_page) query.set("per_page", String(params.per_page));
  const qs = query.toString();
  return request<{ data: import("@/types").GitHubPullRequest[] }>(
    `/api/projects/${projectId}/repositories/${repoId}/pulls${qs ? `?${qs}` : ""}`
  );
}
