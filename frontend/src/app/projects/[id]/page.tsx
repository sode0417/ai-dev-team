"use client";

import { useEffect, useState, use } from "react";
import Link from "next/link";
import { fetchProject, fetchTasks, addRepository } from "@/lib/api";
import type { Project, Task } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";

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

  const load = () => {
    fetchProject(id)
      .then((res) => setProject(res.data))
      .catch((e) => setError(e.message));
    fetchTasks({ project_id: id })
      .then((res) => setTasks(res.data))
      .catch(() => {});
  };

  useEffect(() => {
    load();
  }, [id]);

  if (!project) {
    return <div className="text-slate-500">読み込み中...</div>;
  }

  return (
    <div>
      <h2 className="text-2xl font-bold mb-2">{project.name}</h2>
      {project.description && (
        <p className="text-slate-500 mb-6">{project.description}</p>
      )}

      {error && <div className="text-red-500 mb-4 text-sm">{error}</div>}

      {/* Repositories */}
      <div className="mb-8">
        <div className="flex items-center gap-3 mb-3">
          <h3 className="text-lg font-semibold">Repositories</h3>
          <button
            onClick={() => setShowRepoForm(!showRepoForm)}
            className="text-sm text-blue-600 hover:underline"
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
          <p className="text-slate-500 text-sm">リポジトリはまだありません</p>
        ) : (
          <div className="space-y-2">
            {project.repositories.map((repo) => (
              <div
                key={repo.id}
                className="p-3 rounded border border-slate-200 dark:border-slate-700 text-sm"
              >
                <span className="font-medium">
                  {repo.owner}/{repo.name}
                </span>
                <span className="text-slate-500 ml-2">({repo.default_branch})</span>
                {repo.local_path && (
                  <span className="text-slate-400 ml-2 font-mono text-xs">
                    {repo.local_path}
                  </span>
                )}
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Tasks */}
      <div>
        <h3 className="text-lg font-semibold mb-3">Tasks</h3>
        {tasks.length === 0 ? (
          <p className="text-slate-500 text-sm">タスクはまだありません</p>
        ) : (
          <div className="space-y-2">
            {tasks.map((task) => (
              <Link
                key={task.id}
                href={`/tasks/${task.id}`}
                className="block p-3 rounded border border-slate-200 hover:border-slate-400 transition dark:border-slate-700"
              >
                <div className="flex items-center gap-3">
                  <StatusBadge status={task.status} />
                  <span className="font-medium">{task.title}</span>
                </div>
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

  return (
    <form
      onSubmit={handleSubmit}
      className="mb-4 p-3 border border-slate-200 rounded dark:border-slate-700 space-y-2"
    >
      <div className="flex gap-2">
        <input
          type="text"
          value={owner}
          onChange={(e) => setOwner(e.target.value)}
          placeholder="owner"
          required
          className="flex-1 px-2 py-1 border rounded text-sm dark:bg-slate-800 dark:border-slate-600"
        />
        <input
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          placeholder="repo name"
          required
          className="flex-1 px-2 py-1 border rounded text-sm dark:bg-slate-800 dark:border-slate-600"
        />
      </div>
      <input
        type="text"
        value={localPath}
        onChange={(e) => setLocalPath(e.target.value)}
        placeholder="local path (e.g. /Users/naoto/Projects/repo)"
        className="w-full px-2 py-1 border rounded text-sm dark:bg-slate-800 dark:border-slate-600"
      />
      <button
        type="submit"
        disabled={submitting}
        className="px-3 py-1 bg-blue-600 text-white rounded text-sm disabled:opacity-50"
      >
        {submitting ? "追加中..." : "追加"}
      </button>
    </form>
  );
}
