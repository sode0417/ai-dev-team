"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { fetchTasks, fetchProjects, createTask } from "@/lib/api";
import type { Task, Project } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";
import { PriorityBadge } from "@/components/PriorityBadge";

export default function TasksPage() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [showForm, setShowForm] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const load = () => {
    fetchTasks()
      .then((res) => setTasks(res.data))
      .catch((e) => setError(e.message));
    fetchProjects()
      .then((res) => setProjects(res.data))
      .catch(() => {});
  };

  useEffect(() => { load(); }, []);

  return (
    <div>
      <div className="flex items-center justify-between mb-6">
        <h2 className="text-xl font-semibold">Tasks</h2>
        <button
          onClick={() => setShowForm(!showForm)}
          className="px-3 py-1.5 bg-gh-green/90 text-white rounded-md hover:bg-gh-green transition text-sm font-medium"
        >
          + New Task
        </button>
      </div>

      {error && <div className="text-gh-red mb-4 text-sm">{error}</div>}

      {showForm && (
        <CreateTaskForm
          projects={projects}
          onCreated={() => {
            setShowForm(false);
            load();
          }}
        />
      )}

      {tasks.length === 0 ? (
        <p className="text-gh-text-secondary text-sm">タスクはまだありません</p>
      ) : (
        <div className="rounded-lg border border-gh-border overflow-hidden">
          {tasks.map((task, i) => (
            <Link
              key={task.id}
              href={`/tasks/${task.id}`}
              className={`block px-4 py-3 hover:bg-gh-surface transition ${
                i > 0 ? "border-t border-gh-border" : ""
              }`}
            >
              <div className="flex items-center gap-2.5">
                <StatusBadge status={task.status} />
                <PriorityBadge priority={task.priority} />
                <span className="font-medium text-sm">{task.title}</span>
                {task.pr_url && (
                  <span className="text-xs text-gh-purple font-medium ml-1">PR</span>
                )}
                <span className="text-xs text-gh-text-muted ml-auto shrink-0">
                  {new Date(task.updated_at).toLocaleString("ja-JP")}
                </span>
              </div>
              <p className="text-xs text-gh-text-secondary mt-1 line-clamp-1 ml-0.5">
                {task.description}
              </p>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}

function CreateTaskForm({
  projects,
  onCreated,
}: {
  projects: Project[];
  onCreated: () => void;
}) {
  const [projectId, setProjectId] = useState("");
  const [repoId, setRepoId] = useState("");
  const [title, setTitle] = useState("");
  const [description, setDescription] = useState("");
  const [priority, setPriority] = useState("medium");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const selectedProject = projects.find((p) => p.id === projectId);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setSubmitting(true);
    setError(null);
    try {
      await createTask({
        project_id: projectId,
        repository_id: repoId || undefined,
        title,
        description,
        priority,
      });
      onCreated();
    } catch (err) {
      setError(err instanceof Error ? err.message : "Failed to create task");
    } finally {
      setSubmitting(false);
    }
  };

  const inputClass =
    "w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40";

  return (
    <form
      onSubmit={handleSubmit}
      className="mb-6 p-4 bg-gh-surface border border-gh-border rounded-lg space-y-3"
    >
      {error && <div className="text-gh-red text-sm">{error}</div>}
      <div>
        <label className="block text-xs font-medium text-gh-text-secondary mb-1">Project</label>
        <select
          value={projectId}
          onChange={(e) => {
            setProjectId(e.target.value);
            setRepoId("");
          }}
          required
          className={inputClass}
        >
          <option value="">選択してください</option>
          {projects.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </div>
      {selectedProject && selectedProject.repositories.length > 0 && (
        <div>
          <label className="block text-xs font-medium text-gh-text-secondary mb-1">Repository</label>
          <select value={repoId} onChange={(e) => setRepoId(e.target.value)} className={inputClass}>
            <option value="">（なし）</option>
            {selectedProject.repositories.map((r) => (
              <option key={r.id} value={r.id}>
                {r.owner}/{r.name}
              </option>
            ))}
          </select>
        </div>
      )}
      <div>
        <label className="block text-xs font-medium text-gh-text-secondary mb-1">Title</label>
        <input type="text" value={title} onChange={(e) => setTitle(e.target.value)} required className={inputClass} />
      </div>
      <div>
        <label className="block text-xs font-medium text-gh-text-secondary mb-1">Description</label>
        <textarea value={description} onChange={(e) => setDescription(e.target.value)} required rows={3} className={inputClass} />
      </div>
      <div>
        <label className="block text-xs font-medium text-gh-text-secondary mb-1">Priority</label>
        <select value={priority} onChange={(e) => setPriority(e.target.value)} className={inputClass}>
          <option value="low">Low</option>
          <option value="medium">Medium</option>
          <option value="high">High</option>
          <option value="critical">Critical</option>
        </select>
      </div>
      <button
        type="submit"
        disabled={submitting}
        className="px-4 py-2 bg-gh-green/90 text-white rounded-md hover:bg-gh-green transition text-sm font-medium disabled:opacity-50"
      >
        {submitting ? "作成中..." : "作成"}
      </button>
    </form>
  );
}
