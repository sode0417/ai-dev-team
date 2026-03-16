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
      <div className="text-red-500">
        Backend に接続できません: {error}
      </div>
    );
  }

  if (!data) {
    return <div className="text-slate-500">読み込み中...</div>;
  }

  return (
    <div>
      <h2 className="text-2xl font-bold mb-6">Dashboard</h2>

      <div className="grid grid-cols-2 md:grid-cols-4 gap-4 mb-8">
        <StatCard label="Projects" value={data.total_projects} />
        <StatCard label="Total Tasks" value={data.total_tasks} />
        <StatCard label="Active" value={data.active_tasks} color="blue" />
        <StatCard label="Completed" value={data.completed_tasks} color="green" />
      </div>

      <h3 className="text-lg font-semibold mb-3">Recent Tasks</h3>
      {data.recent_tasks.length === 0 ? (
        <p className="text-slate-500">タスクはまだありません</p>
      ) : (
        <div className="space-y-2">
          {data.recent_tasks.map((task: Task) => (
            <Link
              key={task.id}
              href={`/tasks/${task.id}`}
              className="block p-3 rounded border border-slate-200 hover:border-slate-400 transition dark:border-slate-700"
            >
              <div className="flex items-center gap-3">
                <StatusBadge status={task.status} />
                <span className="font-medium">{task.title}</span>
                <span className="text-sm text-slate-500 ml-auto">
                  {new Date(task.updated_at).toLocaleString("ja-JP")}
                </span>
              </div>
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
  const colorClass =
    color === "blue"
      ? "text-blue-600"
      : color === "green"
        ? "text-green-600"
        : "text-slate-900 dark:text-slate-100";

  return (
    <div className="p-4 rounded-lg border border-slate-200 dark:border-slate-700">
      <div className="text-sm text-slate-500">{label}</div>
      <div className={`text-3xl font-bold ${colorClass}`}>{value}</div>
    </div>
  );
}
