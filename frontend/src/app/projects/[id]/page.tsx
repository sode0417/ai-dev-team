"use client";

import { useEffect, useState, useCallback, use } from "react";
import { useSearchParams } from "next/navigation";
import Link from "next/link";
import {
  fetchProject,
  fetchTasks,
  addRepository,
  updateProject,
  createSprint,
  fetchSprints,
  fetchActiveSprint,
  fetchRepositoryIssues,
  approveTask,
  executeTask,
  cancelTask,
  executeIssue,
} from "@/lib/api";
import type { Project, ProjectRepository, Task, Sprint, GitHubIssue } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";
import { PriorityBadge } from "@/components/PriorityBadge";
import { IssueList } from "@/components/IssueList";
import { PullRequestList } from "@/components/PullRequestList";
import { SprintPanel } from "@/components/SprintPanel";

type PageTab = "sprint" | "tasks" | "repositories";

export default function ProjectDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const searchParams = useSearchParams();
  const initialTab = (searchParams.get("tab") as PageTab) || "sprint";

  const [project, setProject] = useState<Project | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [sprints, setSprints] = useState<Sprint[]>([]);
  const [activeSprintId, setActiveSprintId] = useState<string | null>(null);
  const [showRepoForm, setShowRepoForm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<PageTab>(initialTab);
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);
  const [repoTab, setRepoTab] = useState<"issues" | "pulls">("issues");
  const [issueCounts, setIssueCounts] = useState<Record<string, number>>({});
  const [creatingSprint, setCreatingSprint] = useState(false);

  const loadTasks = useCallback(() => {
    fetchTasks({ project_id: id })
      .then((res) => setTasks(res.data))
      .catch(() => {});
  }, [id]);

  const loadSprints = useCallback(() => {
    fetchSprints(id)
      .then((res) => setSprints(res.data))
      .catch(() => {});
    fetchActiveSprint(id)
      .then((res) => {
        setActiveSprintId(res.data ? res.data.id : null);
      })
      .catch(() => {});
  }, [id]);

  const load = useCallback(() => {
    fetchProject(id)
      .then((res) => {
        setProject(res.data);
        if (res.data.repositories.length > 0) {
          setSelectedRepoId((prev) => prev || res.data.repositories[0].id);
        }
        for (const repo of res.data.repositories) {
          fetchRepositoryIssues(id, repo.id, { state: "open", per_page: 100 })
            .then((r) => {
              setIssueCounts((prev) => ({ ...prev, [repo.id]: r.data.length }));
            })
            .catch(() => {});
        }
      })
      .catch((e) => setError(e.message));
    loadTasks();
    loadSprints();
  }, [id, loadTasks, loadSprints]);

  useEffect(() => {
    load();
  }, [load]);

  const handleNewSprint = async () => {
    setCreatingSprint(true);
    setError(null);
    try {
      const res = await createSprint(id);
      setActiveSprintId(res.data.sprint_id);
      setActiveTab("sprint");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Sprint creation failed");
    } finally {
      setCreatingSprint(false);
    }
  };

  const selectedRepo = project?.repositories.find(
    (r) => r.id === selectedRepoId
  );

  if (!project) {
    return <div className="text-gh-text-secondary">読み込み中...</div>;
  }

  const tabClass = (tab: PageTab) =>
    `px-4 py-2 text-sm font-medium border-b-2 transition cursor-pointer ${
      activeTab === tab
        ? "border-gh-blue text-gh-text"
        : "border-transparent text-gh-text-secondary hover:text-gh-text hover:border-gh-border"
    }`;

  return (
    <div>
      {/* Header — inline editable */}
      <ProjectHeader
        project={project}
        onUpdated={(p) => setProject({ ...project, ...p })}
      />
      <div className="flex items-center justify-end mb-2">
        <button
          onClick={handleNewSprint}
          disabled={creatingSprint || !!activeSprintId}
          className="px-3 py-1.5 bg-gh-purple/90 text-white rounded-md hover:bg-gh-purple transition text-sm font-medium disabled:opacity-50"
          title={activeSprintId ? "アクティブなスプリントがあります" : ""}
        >
          {creatingSprint ? "作成中..." : activeSprintId ? "Sprint 進行中" : "New Sprint"}
        </button>
      </div>

      {error && <div className="text-gh-red mb-4 text-sm">{error}</div>}

      {/* Tabs */}
      <div className="flex border-b border-gh-border mb-4 -mx-4 px-4 sm:-mx-6 sm:px-6 lg:-mx-6 lg:px-6 overflow-x-auto">
        <button className={tabClass("sprint")} onClick={() => setActiveTab("sprint")}>
          Sprint
          {activeSprintId && (
            <span className="ml-1.5 w-2 h-2 rounded-full bg-gh-green inline-block animate-pulse" />
          )}
        </button>
        <button className={tabClass("tasks")} onClick={() => setActiveTab("tasks")}>
          Tasks
          <span className="ml-1.5 text-xs text-gh-text-muted">{tasks.length}</span>
        </button>
        <button className={tabClass("repositories")} onClick={() => setActiveTab("repositories")}>
          Repositories
          <span className="ml-1.5 text-xs text-gh-text-muted">{project.repositories.length}</span>
        </button>
      </div>

      {/* Tab Content — hidden で切り替え（タブ切替時の再フェッチを防止） */}
      <div className={activeTab !== "sprint" ? "hidden" : undefined}>
        <SprintTab
          sprints={sprints}
          activeSprintId={activeSprintId}
          onRefresh={() => { loadTasks(); loadSprints(); }}
        />
      </div>

      <div className={activeTab !== "tasks" ? "hidden" : undefined}>
        <TasksTab tasks={tasks} activeSprintId={activeSprintId} onRefresh={loadTasks} />
      </div>

      <div className={activeTab !== "repositories" ? "hidden" : undefined}>
        <RepositoriesTab
          projectId={id}
          repositories={project.repositories}
          issueCounts={issueCounts}
          selectedRepoId={selectedRepoId}
          repoTab={repoTab}
          selectedRepo={selectedRepo}
          showRepoForm={showRepoForm}
          onSelectRepo={(repoId) => {
            setSelectedRepoId(repoId === selectedRepoId ? null : repoId);
            setRepoTab("issues");
          }}
          onSetRepoTab={setRepoTab}
          onToggleRepoForm={() => setShowRepoForm(!showRepoForm)}
          onRepoAdded={() => { setShowRepoForm(false); load(); }}
          onExecuteIssue={async (repoId, issue) => {
            try {
              await executeIssue({
                project_id: id,
                repository_id: repoId,
                issue_number: issue.number,
                issue_url: issue.html_url,
              });
              loadTasks();
              setActiveTab("tasks");
            } catch (e) {
              setError(e instanceof Error ? e.message : "Issue execution failed");
            }
          }}
        />
      </div>
    </div>
  );
}

/* ─── Project Header (inline editable) ─── */

function ProjectHeader({
  project,
  onUpdated,
}: {
  project: Project;
  onUpdated: (p: { name: string; description: string | null }) => void;
}) {
  const [editing, setEditing] = useState(false);
  const [name, setName] = useState(project.name);
  const [description, setDescription] = useState(project.description || "");
  const [saving, setSaving] = useState(false);

  const handleSave = async () => {
    if (!name.trim()) return;
    setSaving(true);
    try {
      await updateProject(project.id, {
        name: name.trim(),
        description: description.trim() || undefined,
      });
      onUpdated({ name: name.trim(), description: description.trim() || null });
      setEditing(false);
    } catch {
      // keep editing on error
    } finally {
      setSaving(false);
    }
  };

  const handleCancel = () => {
    setName(project.name);
    setDescription(project.description || "");
    setEditing(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSave();
    }
    if (e.key === "Escape") handleCancel();
  };

  if (editing) {
    return (
      <div className="mb-2 space-y-2">
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={handleKeyDown}
          autoFocus
          className="w-full text-xl font-semibold bg-gh-canvas border border-gh-border rounded-md px-3 py-1.5 text-gh-text focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40"
        />
        <input
          type="text"
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          onKeyDown={handleKeyDown}
          placeholder="Description (optional)"
          className="w-full text-sm bg-gh-canvas border border-gh-border rounded-md px-3 py-1.5 text-gh-text-secondary focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40"
        />
        <div className="flex gap-2">
          <button
            onClick={handleSave}
            disabled={saving || !name.trim()}
            className="px-2.5 py-1 text-xs font-medium rounded-md bg-gh-green/15 text-gh-green hover:bg-gh-green/25 transition disabled:opacity-50"
          >
            {saving ? "保存中..." : "保存"}
          </button>
          <button
            onClick={handleCancel}
            className="px-2.5 py-1 text-xs font-medium rounded-md text-gh-text-secondary hover:bg-gh-overlay transition"
          >
            キャンセル
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="group mb-1">
      <div className="flex items-center gap-2">
        <h2 className="text-xl font-semibold">{project.name}</h2>
        <button
          onClick={() => setEditing(true)}
          className="opacity-0 group-hover:opacity-100 text-gh-text-muted hover:text-gh-text transition p-1"
          title="編集"
        >
          <svg className="w-3.5 h-3.5" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
            <path strokeLinecap="round" strokeLinejoin="round" d="m16.862 4.487 1.687-1.688a1.875 1.875 0 1 1 2.652 2.652L10.582 16.07a4.5 4.5 0 0 1-1.897 1.13L6 18l.8-2.685a4.5 4.5 0 0 1 1.13-1.897l8.932-8.931Z" />
          </svg>
        </button>
      </div>
      {project.description && (
        <p className="text-gh-text-secondary text-sm mt-0.5">
          {project.description}
        </p>
      )}
    </div>
  );
}

/* ─── Repositories Tab ─── */

function RepositoriesTab({
  projectId,
  repositories,
  issueCounts,
  selectedRepoId,
  repoTab,
  selectedRepo,
  showRepoForm,
  onSelectRepo,
  onSetRepoTab,
  onToggleRepoForm,
  onRepoAdded,
  onExecuteIssue,
}: {
  projectId: string;
  repositories: ProjectRepository[];
  issueCounts: Record<string, number>;
  selectedRepoId: string | null;
  repoTab: "issues" | "pulls";
  selectedRepo: ProjectRepository | undefined;
  showRepoForm: boolean;
  onSelectRepo: (id: string) => void;
  onSetRepoTab: (tab: "issues" | "pulls") => void;
  onToggleRepoForm: () => void;
  onRepoAdded: () => void;
  onExecuteIssue?: (repoId: string, issue: GitHubIssue) => void;
}) {
  return (
    <div>
      <div className="flex items-center gap-3 mb-3">
        <button
          onClick={onToggleRepoForm}
          className="text-xs text-gh-link hover:underline"
        >
          + Add Repository
        </button>
      </div>

      {showRepoForm && (
        <AddRepoForm projectId={projectId} onAdded={onRepoAdded} />
      )}

      {repositories.length === 0 ? (
        <p className="text-gh-text-secondary text-sm">リポジトリはまだありません</p>
      ) : (
        <>
          <div className="flex flex-wrap gap-2 mb-3">
            {repositories.map((repo) => (
              <RepoCard
                key={repo.id}
                repo={repo}
                issueCount={issueCounts[repo.id]}
                selected={repo.id === selectedRepoId}
                onClick={() => onSelectRepo(repo.id)}
              />
            ))}
          </div>

          {selectedRepo && (
            <div className="rounded-lg border border-gh-border overflow-hidden">
              <div className="px-4 py-2.5 bg-gh-surface flex flex-col sm:flex-row sm:items-center justify-between gap-2">
                <div className="flex items-center gap-2 min-w-0">
                  <a
                    href={`https://github.com/${selectedRepo.owner}/${selectedRepo.name}`}
                    target="_blank"
                    rel="noopener noreferrer"
                    className="text-sm font-medium text-gh-link hover:underline truncate"
                  >
                    {selectedRepo.owner}/{selectedRepo.name}
                  </a>
                  <span className="text-xs text-gh-text-muted px-1.5 py-0.5 rounded bg-gh-border/30 shrink-0">
                    {selectedRepo.default_branch}
                  </span>
                </div>
                <div className="flex gap-1 shrink-0">
                  <button
                    onClick={() => onSetRepoTab("issues")}
                    className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
                      repoTab === "issues"
                        ? "bg-gh-green/15 text-gh-green"
                        : "text-gh-text-secondary hover:bg-gh-overlay hover:text-gh-text"
                    }`}
                  >
                    Issues
                  </button>
                  <button
                    onClick={() => onSetRepoTab("pulls")}
                    className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
                      repoTab === "pulls"
                        ? "bg-gh-purple/15 text-gh-purple"
                        : "text-gh-text-secondary hover:bg-gh-overlay hover:text-gh-text"
                    }`}
                  >
                    Pull Requests
                  </button>
                </div>
              </div>
              <div className="border-t border-gh-border px-4 pb-3">
                {repoTab === "issues" ? (
                  <IssueList
                    projectId={projectId}
                    repoId={selectedRepo.id}
                    onExecute={onExecuteIssue ? (issue) => onExecuteIssue(selectedRepo.id, issue) : undefined}
                  />
                ) : (
                  <PullRequestList projectId={projectId} repoId={selectedRepo.id} />
                )}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}

/* ─── Tasks Tab ─── */

const STATUS_FILTERS = [
  { value: "active", label: "Active", match: (s: string) => !["completed", "failed", "cancelled"].includes(s) },
  { value: "completed", label: "Completed", match: (s: string) => s === "completed" },
  { value: "failed", label: "Failed", match: (s: string) => s === "failed" },
  { value: "cancelled", label: "Cancelled", match: (s: string) => s === "cancelled" },
  { value: "all", label: "All", match: () => true },
] as const;

type StatusFilter = (typeof STATUS_FILTERS)[number]["value"];
type SprintFilter = "current" | "all";

function TasksTab({
  tasks,
  activeSprintId,
  onRefresh,
}: {
  tasks: Task[];
  activeSprintId: string | null;
  onRefresh: () => void;
}) {
  const [filter, setFilter] = useState<StatusFilter>("active");
  const [sprintFilter, setSprintFilter] = useState<SprintFilter>(activeSprintId ? "current" : "all");

  const handleAction = async (action: "approve" | "execute" | "execute-skip" | "cancel", taskId: string) => {
    try {
      if (action === "approve") await approveTask(taskId);
      else if (action === "execute") await executeTask(taskId, false);
      else if (action === "execute-skip") await executeTask(taskId, true);
      else await cancelTask(taskId);
      onRefresh();
    } catch (e) {
      console.error(`Failed to ${action} task:`, e);
    }
  };

  // Sprint filter first, then status filter
  const sprintFiltered = sprintFilter === "current" && activeSprintId
    ? tasks.filter((t) => t.sprint_id === activeSprintId)
    : tasks;

  const currentFilter = STATUS_FILTERS.find((f) => f.value === filter)!;
  const filteredTasks = sprintFiltered.filter((t) => currentFilter.match(t.status));

  const counts: Record<StatusFilter, number> = {
    active: sprintFiltered.filter((t) => STATUS_FILTERS[0].match(t.status)).length,
    completed: sprintFiltered.filter((t) => t.status === "completed").length,
    failed: sprintFiltered.filter((t) => t.status === "failed").length,
    cancelled: sprintFiltered.filter((t) => t.status === "cancelled").length,
    all: sprintFiltered.length,
  };

  if (tasks.length === 0) {
    return <p className="text-gh-text-secondary text-sm">タスクはまだありません</p>;
  }

  const sprintBtnClass = (v: SprintFilter) =>
    `px-2.5 py-1 text-xs font-medium rounded-md transition cursor-pointer ${
      sprintFilter === v
        ? "bg-gh-purple/15 text-gh-purple"
        : "text-gh-text-secondary hover:bg-gh-overlay hover:text-gh-text"
    }`;

  return (
    <div>
      {/* Sprint filter + Status filter */}
      <div className="flex items-center gap-3 mb-3 flex-wrap">
        {activeSprintId && (
          <div className="flex gap-1 pr-3 border-r border-gh-border">
            <button className={sprintBtnClass("current")} onClick={() => setSprintFilter("current")}>
              Current Sprint
            </button>
            <button className={sprintBtnClass("all")} onClick={() => setSprintFilter("all")}>
              All
            </button>
          </div>
        )}
        <div className="flex gap-1 flex-wrap">
          {STATUS_FILTERS.map((f) => (
            <button
              key={f.value}
              onClick={() => setFilter(f.value)}
              className={`px-2.5 py-1 text-xs font-medium rounded-md transition cursor-pointer ${
                filter === f.value
                  ? "bg-gh-blue/15 text-gh-blue"
                  : "text-gh-text-secondary hover:bg-gh-overlay hover:text-gh-text"
              }`}
            >
              {f.label}
              {counts[f.value] > 0 && (
                <span className="ml-1 text-[10px] opacity-70">{counts[f.value]}</span>
              )}
            </button>
          ))}
        </div>
      </div>

      {filteredTasks.length === 0 ? (
        <p className="text-gh-text-secondary text-sm py-4 text-center">該当するタスクはありません</p>
      ) : (
    <div className="rounded-lg border border-gh-border overflow-hidden">
      {filteredTasks.map((task, i) => (
        <div
          key={task.id}
          className={`px-4 py-3 ${i > 0 ? "border-t border-gh-border" : ""}`}
        >
          <div className="flex items-start gap-3">
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2 flex-wrap mb-0.5">
                <StatusBadge status={task.status} />
                <PriorityBadge priority={task.priority} />
                {task.proposal_type !== "development" && (
                  <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                    task.proposal_type === "improvement"
                      ? "bg-gh-orange/15 text-gh-orange"
                      : task.proposal_type === "operation"
                      ? "bg-gh-green/15 text-gh-green"
                      : "bg-gh-purple/15 text-gh-purple"
                  }`}>
                    {task.proposal_type}
                  </span>
                )}
              </div>
              <Link
                href={`/tasks/${task.id}`}
                className="text-sm font-medium hover:text-gh-link transition"
              >
                {task.title}
              </Link>
              <p className="text-xs text-gh-text-muted mt-0.5 line-clamp-1">
                {task.description}
              </p>
              {task.pr_url && (
                <a
                  href={task.pr_url}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-xs text-gh-link hover:underline mt-1 inline-block"
                >
                  {task.pr_url}
                </a>
              )}
            </div>
            <div className="flex gap-1.5 shrink-0 mt-0.5">
              {task.status === "proposed" && (
                <>
                  <button
                    onClick={() => handleAction("approve", task.id)}
                    className="px-2 py-1 text-xs font-medium rounded-md bg-gh-green/15 text-gh-green hover:bg-gh-green/25 transition"
                  >
                    Approve
                  </button>
                  <button
                    onClick={() => handleAction("cancel", task.id)}
                    className="px-2 py-1 text-xs font-medium rounded-md bg-gh-text-muted/15 text-gh-text-secondary hover:bg-gh-text-muted/25 transition"
                  >
                    Dismiss
                  </button>
                </>
              )}
              {task.status === "approved" && (
                <>
                  <button
                    onClick={() => handleAction("execute", task.id)}
                    className="px-2 py-1 text-xs font-medium rounded-md bg-gh-blue/15 text-gh-blue hover:bg-gh-blue/25 transition"
                  >
                    Execute
                  </button>
                  <button
                    onClick={() => handleAction("execute-skip", task.id)}
                    className="px-2 py-1 text-xs font-medium rounded-md bg-gh-text-muted/15 text-gh-text-secondary hover:bg-gh-text-muted/25 transition"
                    title="即時実行（ヒアリング・計画承認スキップ）"
                  >
                    即時
                  </button>
                </>
              )}
            </div>
          </div>
        </div>
      ))}
    </div>
      )}
    </div>
  );
}

/* ─── Sprint Tab ─── */

function SprintTab({
  sprints,
  activeSprintId,
  onRefresh,
}: {
  sprints: Sprint[];
  activeSprintId: string | null;
  onRefresh: () => void;
}) {
  const [selectedSprintId, setSelectedSprintId] = useState<string | null>(
    activeSprintId
  );

  useEffect(() => {
    if (activeSprintId) setSelectedSprintId(activeSprintId);
  }, [activeSprintId]);

  const viewSprintId = selectedSprintId || activeSprintId;

  return (
    <div>
      {/* Active sprint */}
      {viewSprintId && (
        <div className="mb-6">
          <SprintPanel sprintId={viewSprintId} onRefresh={onRefresh} />
        </div>
      )}

      {/* Sprint History */}
      {sprints.length > 0 && (
        <div>
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider mb-2">
            Sprint History
          </h4>
          <div className="rounded-lg border border-gh-border overflow-hidden">
            {sprints.map((sprint, i) => (
              <button
                key={sprint.id}
                onClick={() => setSelectedSprintId(sprint.id)}
                className={`w-full text-left px-4 py-2.5 transition cursor-pointer ${
                  i > 0 ? "border-t border-gh-border" : ""
                } ${
                  sprint.id === viewSprintId
                    ? "bg-gh-blue/5"
                    : "hover:bg-gh-surface"
                }`}
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span
                      className={`w-2 h-2 rounded-full shrink-0 ${
                        sprint.status === "completed"
                          ? "bg-gh-green"
                          : sprint.status === "failed"
                          ? "bg-gh-red"
                          : "bg-gh-purple animate-pulse"
                      }`}
                    />
                    <span className="text-sm text-gh-text-secondary">
                      {new Date(sprint.created_at).toLocaleString("ja-JP")}
                    </span>
                    <span className="text-xs text-gh-text-muted">{sprint.status}</span>
                  </div>
                </div>
                {/* Preview for completed sprints */}
                {sprint.status === "completed" && (sprint.scan_analysis || sprint.retrospective) && (
                  <p className="text-xs text-gh-text-muted mt-1 line-clamp-1 pl-4">
                    {sprint.retrospective
                      ? sprint.retrospective.slice(0, 120)
                      : sprint.scan_analysis
                      ? sprint.scan_analysis.slice(0, 120)
                      : ""}
                  </p>
                )}
              </button>
            ))}
          </div>
        </div>
      )}

      {!viewSprintId && sprints.length === 0 && (
        <p className="text-gh-text-secondary text-sm">
          まだスプリントを実行していません。右上の New Sprint ボタンで開始できます。
        </p>
      )}
    </div>
  );
}

/* ─── Shared Components ─── */

function RepoCard({
  repo,
  issueCount,
  selected,
  onClick,
}: {
  repo: ProjectRepository;
  issueCount: number | undefined;
  selected: boolean;
  onClick: () => void;
}) {
  return (
    <button
      onClick={onClick}
      className={`flex items-center gap-2 px-3 py-2 rounded-lg border text-sm transition cursor-pointer ${
        selected
          ? "border-gh-blue bg-gh-blue/10 text-gh-text"
          : "border-gh-border bg-gh-surface text-gh-text-secondary hover:border-gh-text-muted hover:text-gh-text"
      }`}
    >
      <svg
        className="w-3.5 h-3.5 opacity-60 shrink-0"
        fill="none"
        viewBox="0 0 24 24"
        stroke="currentColor"
        strokeWidth={1.5}
      >
        <path
          strokeLinecap="round"
          strokeLinejoin="round"
          d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z"
        />
      </svg>
      <span className="font-medium truncate">{repo.name}</span>
      {issueCount !== undefined && issueCount > 0 && (
        <span className="inline-flex items-center justify-center min-w-[18px] h-[18px] px-1 rounded-full bg-gh-green/15 text-gh-green text-[10px] font-bold">
          {issueCount}
        </span>
      )}
    </button>
  );
}

function AddRepoForm({
  projectId,
  onAdded,
}: {
  projectId: string;
  onAdded: () => void;
}) {
  const [owner, setOwner] = useState("sode0417");
  const [name, setName] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [submitting, setSubmitting] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    try {
      await addRepository(projectId, {
        owner,
        name,
        local_path: localPath || undefined,
      });
      onAdded();
    } catch {
      alert("Failed to add repository");
    } finally {
      setSubmitting(false);
    }
  };

  const inputClass =
    "w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40";

  return (
    <form
      onSubmit={handleSubmit}
      className="mb-4 p-4 bg-gh-surface border border-gh-border rounded-lg space-y-2"
    >
      <div className="flex gap-2">
        <input
          type="text"
          value={owner}
          onChange={(e) => setOwner(e.target.value)}
          placeholder="owner"
          required
          className={inputClass}
        />
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="repo name"
          required
          className={inputClass}
        />
      </div>
      <input
        type="text"
        value={localPath}
        onChange={(e) => setLocalPath(e.target.value)}
        placeholder="local path (e.g. /Users/naoto/Projects/repo)"
        className={inputClass}
      />
      <button
        type="submit"
        disabled={submitting}
        className="px-3 py-1.5 bg-gh-green/90 text-white rounded-md hover:bg-gh-green transition text-sm font-medium disabled:opacity-50"
      >
        {submitting ? "追加中..." : "追加"}
      </button>
    </form>
  );
}
