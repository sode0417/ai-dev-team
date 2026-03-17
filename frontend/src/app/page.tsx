"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { fetchDashboard, fetchProjects, fetchActiveSprint } from "@/lib/api";
import type { DashboardData, Task, Project, SprintWithTasks } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";

interface ActiveSprintInfo {
  projectId: string;
  projectName: string;
  sprint: SprintWithTasks;
}

export default function DashboardPage() {
  const [data, setData] = useState<DashboardData | null>(null);
  const [activeSprints, setActiveSprints] = useState<ActiveSprintInfo[]>([]);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchDashboard()
      .then((res) => setData(res.data))
      .catch((e) => setError(e.message));

    // Fetch active sprints for all projects
    fetchProjects()
      .then(async (res) => {
        const results: ActiveSprintInfo[] = [];
        for (const project of res.data) {
          try {
            const sprintRes = await fetchActiveSprint(project.id);
            if (sprintRes.data) {
              results.push({
                projectId: project.id,
                projectName: project.name,
                sprint: sprintRes.data,
              });
            }
          } catch {
            // skip
          }
        }
        setActiveSprints(results);
      })
      .catch(() => {});
  }, []);

  if (error) {
    return (
      <div className="text-gh-red">
        Backend に接続できません: {error}
      </div>
    );
  }

  if (!data) {
    return <div className="text-gh-text-secondary">読み込み中...</div>;
  }

  return (
    <div>
      <h2 className="text-xl font-semibold mb-6">Dashboard</h2>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-3 mb-8">
        <StatCard label="Projects" value={data.total_projects} />
        <StatCard label="Total Tasks" value={data.total_tasks} />
        <StatCard label="Active" value={data.active_tasks} color="blue" />
        <StatCard label="Completed" value={data.completed_tasks} color="green" />
      </div>

      {/* Active Sprints */}
      {activeSprints.length > 0 && (
        <div className="mb-8">
          <h3 className="text-sm font-semibold text-gh-text-secondary uppercase tracking-wider mb-3">
            Active Sprints
          </h3>
          <div className="grid gap-3">
            {activeSprints.map((info) => (
              <ActiveSprintCard key={info.sprint.id} info={info} />
            ))}
          </div>
        </div>
      )}

      <h3 className="text-sm font-semibold text-gh-text-secondary uppercase tracking-wider mb-3">
        Recent Tasks
      </h3>
      {data.recent_tasks.length === 0 ? (
        <p className="text-gh-text-secondary text-sm">タスクはまだありません</p>
      ) : (
        <div className="rounded-lg border border-gh-border overflow-hidden">
          {data.recent_tasks.map((task: Task, i: number) => (
            <Link
              key={task.id}
              href={`/tasks/${task.id}`}
              className={`flex items-center gap-3 px-4 py-3 hover:bg-gh-surface transition ${
                i > 0 ? "border-t border-gh-border" : ""
              }`}
            >
              <StatusBadge status={task.status} />
              <span className="font-medium text-sm">{task.title}</span>
              <span className="text-xs text-gh-text-muted ml-auto">
                {new Date(task.updated_at).toLocaleString("ja-JP")}
              </span>
            </Link>
          ))}
        </div>
      )}
    </div>
  );
}

const SPRINT_STATUS_COLORS: Record<string, { dot: string; bg: string; text: string }> = {
  selecting:      { dot: "bg-gh-orange",  bg: "bg-gh-orange/5",  text: "text-gh-orange" },
  hearing:        { dot: "bg-gh-orange",  bg: "bg-gh-orange/5",  text: "text-gh-orange" },
  planning:       { dot: "bg-gh-blue",    bg: "bg-gh-blue/5",    text: "text-gh-blue" },
  executing:      { dot: "bg-gh-purple",  bg: "bg-gh-purple/5",  text: "text-gh-purple" },
  retrospective:  { dot: "bg-gh-green",   bg: "bg-gh-green/5",   text: "text-gh-green" },
};

function ActiveSprintCard({ info }: { info: ActiveSprintInfo }) {
  const colors = SPRINT_STATUS_COLORS[info.sprint.status] || SPRINT_STATUS_COLORS.selecting;
  const taskCount = info.sprint.tasks?.length || 0;
  const completedCount = info.sprint.tasks?.filter((t) => t.status === "completed").length || 0;

  return (
    <Link
      href={`/projects/${info.projectId}?tab=sprint`}
      className={`block rounded-lg border border-gh-border ${colors.bg} p-4 hover:border-gh-text-muted transition`}
    >
      <div className="flex items-center justify-between mb-1">
        <span className="text-sm font-medium text-gh-text">{info.projectName}</span>
        <div className="flex items-center gap-1.5">
          <span className={`w-2 h-2 rounded-full ${colors.dot} animate-pulse`} />
          <span className={`text-xs font-medium ${colors.text}`}>{info.sprint.status}</span>
        </div>
      </div>
      {taskCount > 0 && (
        <div className="flex items-center gap-2 mt-2">
          <div className="flex-1 h-1.5 bg-gh-border/50 rounded-full overflow-hidden">
            <div
              className="h-full bg-gh-green rounded-full transition-all"
              style={{ width: `${taskCount > 0 ? (completedCount / taskCount) * 100 : 0}%` }}
            />
          </div>
          <span className="text-xs text-gh-text-muted shrink-0">
            {completedCount}/{taskCount}
          </span>
        </div>
      )}
    </Link>
  );
}

function StatCard({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color?: string;
}) {
  const valueClass =
    color === "blue"
      ? "text-gh-blue"
      : color === "green"
        ? "text-gh-green"
        : "text-gh-text";

  return (
    <div className="p-4 rounded-lg bg-gh-surface border border-gh-border">
      <div className="text-xs text-gh-text-secondary mb-1">{label}</div>
      <div className={`text-2xl font-bold ${valueClass}`}>{value}</div>
    </div>
  );
}
