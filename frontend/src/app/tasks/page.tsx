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
        <h2 className="text-2xl font-bold">Tasks</h2>
        <button
          onClick={() => setShowForm(!showForm)}
          className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition text-sm"
        >
          + New Task
        </button>
      </div>

      {error && <div className="text-red-500 mb-4">{error}</div>}

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
        <p className="text-slate-500">タスクはまだありません</p>
      ) : (
        <div className="space-y-2">
          {tasks.map((task) => (
            <Link
              key={task.id}
              href={`/tasks/${task.id}`}
              className="block p-4 rounded border border-slate-200 hover:border-slate-400 transition dark:border-slate-700"
            >
              <div className="flex items-center gap-3">
                <StatusBadge status={task.status} />
                <PriorityBadge priority={task.priority} />
                <span className="font-medium">{task.title}</span>
                {task.pr_url && (
                  <span className="text-sm text-blue-500 ml-2">PR</span>
                )}
                <span className="text-sm text-slate-500 ml-auto">
                  {new Date(task.updated_at).toLocaleString("ja-JP")}
                </span>
              </div>
              <p className="text-sm text-slate-500 mt-1 line-clamp-1">
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

  return (
    <form
      onSubmit={handleSubmit}
      className="mb-6 p-4 border border-slate-200 rounded-lg dark:border-slate-700 space-y-3"
    >
      {error && <div className="text-red-500 text-sm">{error}</div>}
      <div>
        <label className="block text-sm font-medium mb-1">Project</label>
        <select
          value={projectId}
          onChange={(e) => {
            setProjectId(e.target.value);
            setRepoId("");
          }}
          required
          className="w-full px-3 py-2 border rounded dark:bg-slate-800 dark:border-slate-600"
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
          <label className="block text-sm font-medium mb-1">Repository</label>
          <select
            value={repoId}
            onChange={(e) => setRepoId(e.target.value)}
            className="w-full px-3 py-2 border rounded dark:bg-slate-800 dark:border-slate-600"
          >
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
        <label className="block text-sm font-medium mb-1">Title</label>
        <input
          type="text"
          value={title}
          onChange={(e) => setTitle(e.target.value)}
          required
          className="w-full px-3 py-2 border rounded dark:bg-slate-800 dark:border-slate-600"
        />
      </div>
      <div>
        <label className="block text-sm font-medium mb-1">Description</label>
        <textarea
          value={description}
          onChange={(e) => setDescription(e.target.value)}
          required
          rows={3}
          className="w-full px-3 py-2 border rounded dark:bg-slate-800 dark:border-slate-600"
        />
      </div>
      <div>
        <label className="block text-sm font-medium mb-1">Priority</label>
        <select
          value={priority}
          onChange={(e) => setPriority(e.target.value)}
          className="w-full px-3 py-2 border rounded dark:bg-slate-800 dark:border-slate-600"
        >
          <option value="low">Low</option>
          <option value="medium">Medium</option>
          <option value="high">High</option>
          <option value="critical">Critical</option>
        </select>
      </div>
      <button
        type="submit"
        disabled={submitting}
        className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 transition text-sm disabled:opacity-50"
      >
        {submitting ? "作成中..." : "作成"}
      </button>
    </form>
  );
}
