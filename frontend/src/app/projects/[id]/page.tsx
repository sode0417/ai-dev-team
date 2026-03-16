"use client";

import { useEffect, useState, use } from "react";
import Link from "next/link";
import { fetchProject, fetchTasks, addRepository } from "@/lib/api";
import type { Project, Task } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";
import { IssueList } from "@/components/IssueList";
import { PullRequestList } from "@/components/PullRequestList";

export default function ProjectDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const [project, setProject] = useState<Project | null>(null);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [showRepoForm, setShowRepoForm] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [expandedRepo, setExpandedRepo] = useState<Record<string, "issues" | "pulls" | null>>({});

  const load = () => {
    fetchProject(id)
      .then((res) => {
        setProject(res.data);
        // 初回読み込み時、全リポで Issues をデフォルト展開
        const defaults: Record<string, "issues" | "pulls" | null> = {};
        for (const repo of res.data.repositories) {
          defaults[repo.id] = "issues";
        }
        setExpandedRepo((prev) => {
          const hasEntries = Object.keys(prev).length > 0;
          return hasEntries ? prev : defaults;
        });
      })
      .catch((e) => setError(e.message));
    fetchTasks({ project_id: id })
      .then((res) => setTasks(res.data))
      .catch(() => {});
  };

  useEffect(() => {
    load();
  }, [id]);

  const toggleRepoPanel = (repoId: string, panel: "issues" | "pulls") => {
    setExpandedRepo((prev) => ({
      ...prev,
      [repoId]: prev[repoId] === panel ? null : panel,
    }));
  };

  if (!project) {
    return <div className="text-gh-text-secondary">読み込み中...</div>;
  }

  return (
    <div>
      <h2 className="text-xl font-semibold mb-1">{project.name}</h2>
      {project.description && (
        <p className="text-gh-text-secondary text-sm mb-6">{project.description}</p>
      )}

      {error && <div className="text-gh-red mb-4 text-sm">{error}</div>}

      {/* Repositories */}
      <div className="mb-8">
        <div className="flex items-center gap-3 mb-3">
          <h3 className="text-sm font-semibold text-gh-text-secondary uppercase tracking-wider">
            Repositories
          </h3>
          <button
            onClick={() => setShowRepoForm(!showRepoForm)}
            className="text-xs text-gh-link hover:underline"
          >
            + Add
          </button>
        </div>

        {showRepoForm && (
          <AddRepoForm
            projectId={id}
            onAdded={() => {
              setShowRepoForm(false);
              load();
            }}
          />
        )}

        {project.repositories.length === 0 ? (
          <p className="text-gh-text-secondary text-sm">リポジトリはまだありません</p>
        ) : (
          <div className="space-y-3">
            {project.repositories.map((repo) => (
              <div
                key={repo.id}
                className="rounded-lg border border-gh-border overflow-hidden"
              >
                <div className="px-4 py-3 bg-gh-surface flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <svg className="w-4 h-4 text-gh-text-secondary" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={1.5}>
                      <path strokeLinecap="round" strokeLinejoin="round" d="M2.25 12.75V12A2.25 2.25 0 0 1 4.5 9.75h15A2.25 2.25 0 0 1 21.75 12v.75m-8.69-6.44-2.12-2.12a1.5 1.5 0 0 0-1.061-.44H4.5A2.25 2.25 0 0 0 2.25 6v12a2.25 2.25 0 0 0 2.25 2.25h15A2.25 2.25 0 0 0 21.75 18V9a2.25 2.25 0 0 0-2.25-2.25h-5.379a1.5 1.5 0 0 1-1.06-.44Z" />
                    </svg>
                    <a
                      href={`https://github.com/${repo.owner}/${repo.name}`}
                      target="_blank"
                      rel="noopener noreferrer"
                      className="text-sm font-medium text-gh-link hover:underline"
                    >
                      {repo.owner}/{repo.name}
                    </a>
                    <span className="text-xs text-gh-text-muted px-1.5 py-0.5 rounded bg-gh-border/30">
                      {repo.default_branch}
                    </span>
                    {repo.local_path && (
                      <span className="text-gh-text-muted font-mono text-[11px] hidden md:inline">
                        {repo.local_path}
                      </span>
                    )}
                  </div>
                  <div className="flex gap-1">
                    <button
                      onClick={() => toggleRepoPanel(repo.id, "issues")}
                      className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
                        expandedRepo[repo.id] === "issues"
                          ? "bg-gh-green/15 text-gh-green"
                          : "text-gh-text-secondary hover:bg-gh-overlay hover:text-gh-text"
                      }`}
                    >
                      Issues
                    </button>
                    <button
                      onClick={() => toggleRepoPanel(repo.id, "pulls")}
                      className={`text-xs px-2.5 py-1 rounded-md font-medium transition ${
                        expandedRepo[repo.id] === "pulls"
                          ? "bg-gh-purple/15 text-gh-purple"
                          : "text-gh-text-secondary hover:bg-gh-overlay hover:text-gh-text"
                      }`}
                    >
                      Pull Requests
                    </button>
                  </div>
                </div>
                {expandedRepo[repo.id] === "issues" && (
                  <div className="border-t border-gh-border px-4 pb-3">
                    <IssueList projectId={id} repoId={repo.id} />
                  </div>
                )}
                {expandedRepo[repo.id] === "pulls" && (
                  <div className="border-t border-gh-border px-4 pb-3">
                    <PullRequestList projectId={id} repoId={repo.id} />
                  </div>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Tasks */}
      <div>
        <h3 className="text-sm font-semibold text-gh-text-secondary uppercase tracking-wider mb-3">
          Tasks
        </h3>
        {tasks.length === 0 ? (
          <p className="text-gh-text-secondary text-sm">タスクはまだありません</p>
        ) : (
          <div className="rounded-lg border border-gh-border overflow-hidden">
            {tasks.map((task, i) => (
              <Link
                key={task.id}
                href={`/tasks/${task.id}`}
                className={`flex items-center gap-3 px-4 py-3 hover:bg-gh-surface transition ${
                  i > 0 ? "border-t border-gh-border" : ""
                }`}
              >
                <StatusBadge status={task.status} />
                <span className="font-medium text-sm">{task.title}</span>
              </Link>
            ))}
          </div>
        )}
      </div>
    </div>
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
