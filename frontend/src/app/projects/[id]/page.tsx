"use client";

import { useEffect, useState, useCallback, use } from "react";
import Link from "next/link";
import {
  fetchProject,
  fetchTasks,
  addRepository,
  scanProject,
  fetchScans,
  fetchRepositoryIssues,
  approveTask,
  executeTask,
  cancelTask,
} from "@/lib/api";
import type { Project, ProjectRepository, Task, ScanSession } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";
import { PriorityBadge } from "@/components/PriorityBadge";
import { IssueList } from "@/components/IssueList";
import { PullRequestList } from "@/components/PullRequestList";
import { ScanResultPanel } from "@/components/ScanResultPanel";

type PageTab = "repositories" | "tasks" | "scans";

export default function ProjectDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const [project, setProject] = useState<Project | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [scans, setScans] = useState<ScanSession[]>([]);
  const [showRepoForm, setShowRepoForm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [activeTab, setActiveTab] = useState<PageTab>("repositories");
  const [selectedRepoId, setSelectedRepoId] = useState<string | null>(null);
  const [repoTab, setRepoTab] = useState<"issues" | "pulls">("issues");
  const [issueCounts, setIssueCounts] = useState<Record<string, number>>({});
  const [activeScanId, setActiveScanId] = useState<string | null>(null);
  const [scanning, setScanning] = useState(false);

  const loadTasks = useCallback(() => {
    fetchTasks({ project_id: id })
      .then((res) => setTasks(res.data))
      .catch(() => {});
  }, [id]);

  const loadScans = useCallback(() => {
    fetchScans(id)
      .then((res) => setScans(res.data))
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
    loadScans();
  }, [id, loadTasks, loadScans]);

  useEffect(() => {
    load();
  }, [load]);

  const handleScan = async () => {
    setScanning(true);
    setError(null);
    try {
      const res = await scanProject(id);
      setActiveScanId(res.data.scan_id);
      setActiveTab("scans");
    } catch (e) {
      setError(e instanceof Error ? e.message : "Scan failed");
    } finally {
      setScanning(false);
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
      {/* Header */}
      <div className="flex items-center justify-between mb-1">
        <h2 className="text-xl font-semibold">{project.name}</h2>
        <button
          onClick={handleScan}
          disabled={scanning}
          className="px-3 py-1.5 bg-gh-blue/90 text-white rounded-md hover:bg-gh-blue transition text-sm font-medium disabled:opacity-50"
        >
          {scanning ? "スキャン中..." : "🔍 Scan"}
        </button>
      </div>
      {project.description && (
        <p className="text-gh-text-secondary text-sm mb-4">
          {project.description}
        </p>
      )}

      {error && <div className="text-gh-red mb-4 text-sm">{error}</div>}

      {/* Tabs */}
      <div className="flex border-b border-gh-border mb-4 -mx-4 px-4 sm:-mx-6 sm:px-6 lg:-mx-6 lg:px-6 overflow-x-auto">
        <button className={tabClass("repositories")} onClick={() => setActiveTab("repositories")}>
          Repositories
          <span className="ml-1.5 text-xs text-gh-text-muted">{project.repositories.length}</span>
        </button>
        <button className={tabClass("tasks")} onClick={() => setActiveTab("tasks")}>
          Tasks
          <span className="ml-1.5 text-xs text-gh-text-muted">{tasks.length}</span>
        </button>
        <button className={tabClass("scans")} onClick={() => setActiveTab("scans")}>
          Scans
          <span className="ml-1.5 text-xs text-gh-text-muted">{scans.length}</span>
        </button>
      </div>

      {/* Tab Content */}
      {activeTab === "repositories" && (
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
        />
      )}

      {activeTab === "tasks" && (
        <TasksTab tasks={tasks} onRefresh={loadTasks} />
      )}

      {activeTab === "scans" && (
        <ScansTab
          scans={scans}
          activeScanId={activeScanId}
          onTaskAction={() => { loadTasks(); loadScans(); }}
        />
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
                  <IssueList projectId={projectId} repoId={selectedRepo.id} />
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

function TasksTab({
  tasks,
  onRefresh,
}: {
  tasks: Task[];
  onRefresh: () => void;
}) {
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

  if (tasks.length === 0) {
    return <p className="text-gh-text-secondary text-sm">タスクはまだありません</p>;
  }

  return (
    <div className="rounded-lg border border-gh-border overflow-hidden">
      {tasks.map((task, i) => (
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
  );
}

/* ─── Scans Tab ─── */

function ScansTab({
  scans,
  activeScanId,
  onTaskAction,
}: {
  scans: ScanSession[];
  activeScanId: string | null;
  onTaskAction: () => void;
}) {
  const [selectedScanId, setSelectedScanId] = useState<string | null>(
    activeScanId
  );

  // activeScanId が変わったら追従
  useEffect(() => {
    if (activeScanId) setSelectedScanId(activeScanId);
  }, [activeScanId]);

  const viewScanId = selectedScanId || activeScanId;

  return (
    <div>
      {/* Active scan (進行中) */}
      {viewScanId && (
        <div className="mb-6">
          <ScanResultPanel scanId={viewScanId} onTaskAction={onTaskAction} />
        </div>
      )}

      {/* Scan History */}
      {scans.length > 0 && (
        <div>
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider mb-2">
            Scan History
          </h4>
          <div className="rounded-lg border border-gh-border overflow-hidden">
            {scans.map((scan, i) => (
              <button
                key={scan.id}
                onClick={() => setSelectedScanId(scan.id)}
                className={`w-full text-left px-4 py-2.5 flex items-center justify-between transition cursor-pointer ${
                  i > 0 ? "border-t border-gh-border" : ""
                } ${
                  scan.id === viewScanId
                    ? "bg-gh-blue/5"
                    : "hover:bg-gh-surface"
                }`}
              >
                <div className="flex items-center gap-2">
                  <span
                    className={`w-2 h-2 rounded-full shrink-0 ${
                      scan.status === "completed"
                        ? "bg-gh-green"
                        : scan.status === "failed"
                        ? "bg-gh-red"
                        : "bg-gh-orange animate-pulse"
                    }`}
                  />
                  <span className="text-sm text-gh-text-secondary">
                    {new Date(scan.started_at).toLocaleString("ja-JP")}
                  </span>
                </div>
                <span className="text-xs text-gh-text-muted">{scan.status}</span>
              </button>
            ))}
          </div>
        </div>
      )}

      {!viewScanId && scans.length === 0 && (
        <p className="text-gh-text-secondary text-sm">
          まだスキャンを実行していません。右上の Scan ボタンで開始できます。
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
