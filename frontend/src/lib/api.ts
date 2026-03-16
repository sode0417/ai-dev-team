const API_BASE = process.env.NEXT_PUBLIC_API_URL || "http://localhost:8100";

async function request<T>(path: string, options?: RequestInit): Promise<T> {
  const res = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers: {
      "Content-Type": "application/json",
      ...options?.headers,
    },
  });

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

export function executeTask(id: string) {
  return request<{ data: import("@/types").Task }>(`/api/tasks/${id}/execute`, {
    method: "POST",
  });
}

export function cancelTask(id: string) {
  return request<{ data: import("@/types").Task }>(`/api/tasks/${id}/cancel`, {
    method: "POST",
  });
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
