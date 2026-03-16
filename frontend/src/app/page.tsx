"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { fetchDashboard } from "@/lib/api";
import type { DashboardData, Task } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";

export default function DashboardPage() {
  const [data, setData] = useState<DashboardData | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    fetchDashboard()
      .then((res) => setData(res.data))
      .catch((e) => setError(e.message));
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
