"use client";

import { useEffect, useState, useRef, use } from "react";
import {
  fetchTask,
  fetchExecutions,
  fetchExecutionLogs,
  approveTask,
  executeTask,
  cancelTask,
} from "@/lib/api";
import { connectTaskWs } from "@/lib/ws";
import type { Task, ExecutionSession, ExecutionLog, WsMessage } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";
import { PriorityBadge } from "@/components/PriorityBadge";

export default function TaskDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const [task, setTask] = useState<Task | null>(null);
  const [sessions, setSessions] = useState<ExecutionSession[]>([]);
  const [logs, setLogs] = useState<ExecutionLog[]>([]);
  const [wsMessages, setWsMessages] = useState<WsMessage[]>([]);
  const [selectedSession, setSelectedSession] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);

  const load = async () => {
    try {
      const taskRes = await fetchTask(id);
      setTask(taskRes.data);
      const execRes = await fetchExecutions(id);
      setSessions(execRes.data);
      if (execRes.data.length > 0 && !selectedSession) {
        setSelectedSession(execRes.data[0].id);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  };

  useEffect(() => {
    load();
  }, [id]);

  useEffect(() => {
    if (!task) return;
    const isActive = ["planning", "executing", "reviewing"].includes(task.status);
    if (!isActive) return;

    wsRef.current = connectTaskWs(
      id,
      (msg) => setWsMessages((prev) => [...prev, msg]),
      () => load()
    );

    return () => {
      wsRef.current?.close();
    };
  }, [task?.status]);

  useEffect(() => {
    if (!selectedSession) return;
    fetchExecutionLogs(selectedSession)
      .then((res) => setLogs(res.data))
      .catch(() => {});
  }, [selectedSession]);

  const handleAction = async (action: "approve" | "execute" | "cancel") => {
    try {
      if (action === "approve") await approveTask(id);
      else if (action === "execute") await executeTask(id);
      else if (action === "cancel") await cancelTask(id);
      load();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Action failed");
    }
  };

  if (error && !task) {
    return <div className="text-gh-red">{error}</div>;
  }

  if (!task) {
    return <div className="text-gh-text-secondary">読み込み中...</div>;
  }

  return (
    <div className="max-w-4xl">
      <div className="flex items-center gap-3 mb-4">
        <StatusBadge status={task.status} />
        <PriorityBadge priority={task.priority} />
        <h2 className="text-xl font-semibold">{task.title}</h2>
      </div>

      {error && <div className="text-gh-red mb-4 text-sm">{error}</div>}

      <div className="mb-6 p-4 rounded-lg bg-gh-surface border border-gh-border">
        <p className="whitespace-pre-wrap text-sm">{task.description}</p>
        <div className="mt-3 text-xs text-gh-text-secondary space-y-1">
          <div>作成: {new Date(task.created_at).toLocaleString("ja-JP")}</div>
          {task.started_at && (
            <div>開始: {new Date(task.started_at).toLocaleString("ja-JP")}</div>
          )}
          {task.completed_at && (
            <div>完了: {new Date(task.completed_at).toLocaleString("ja-JP")}</div>
          )}
          {task.pr_url && (
            <div>
              PR:{" "}
              <a
                href={task.pr_url}
                target="_blank"
                rel="noopener noreferrer"
                className="text-gh-link hover:underline"
              >
                {task.pr_url}
              </a>
            </div>
          )}
        </div>
      </div>

      {/* アクションボタン */}
      <div className="flex gap-2 mb-6">
        {task.status === "proposed" && (
          <>
            <button
              onClick={() => handleAction("approve")}
              className="px-3 py-1.5 bg-gh-blue/90 text-white rounded-md hover:bg-gh-blue text-sm font-medium transition"
            >
              Approve
            </button>
            <button
              onClick={() => handleAction("execute")}
              className="px-3 py-1.5 bg-gh-green/90 text-white rounded-md hover:bg-gh-green text-sm font-medium transition"
            >
              Execute Now
            </button>
          </>
        )}
        {task.status === "approved" && (
          <button
            onClick={() => handleAction("execute")}
            className="px-3 py-1.5 bg-gh-green/90 text-white rounded-md hover:bg-gh-green text-sm font-medium transition"
          >
            Execute
          </button>
        )}
        {!["completed", "failed", "cancelled"].includes(task.status) && (
          <button
            onClick={() => handleAction("cancel")}
            className="px-3 py-1.5 border border-gh-red/40 text-gh-red rounded-md hover:bg-gh-red/10 text-sm font-medium transition"
          >
            Cancel
          </button>
        )}
      </div>

      {/* 実行計画 */}
      {task.plan && (
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-gh-text-secondary mb-2">Plan</h3>
          <pre className="p-3 bg-gh-surface border border-gh-border rounded-lg text-sm whitespace-pre-wrap overflow-auto max-h-96 text-gh-text">
            {task.plan}
          </pre>
        </div>
      )}

      {/* エラーログ */}
      {task.error_log && (
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-gh-red mb-2">Error</h3>
          <pre className="p-3 bg-gh-red/5 border border-gh-red/20 rounded-lg text-sm whitespace-pre-wrap text-gh-red">
            {task.error_log}
          </pre>
        </div>
      )}

      {/* リアルタイム進捗 */}
      {wsMessages.length > 0 && (
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-gh-text-secondary mb-2">Live Progress</h3>
          <div className="p-3 bg-gh-surface border border-gh-border rounded-lg space-y-1 max-h-48 overflow-auto">
            {wsMessages.map((msg, i) => (
              <div key={i} className="text-sm">
                <span className="font-mono text-gh-text-muted">[{msg.phase}]</span>{" "}
                <span className="text-gh-text">{msg.message}</span>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 実行セッション */}
      {sessions.length > 0 && (
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-gh-text-secondary mb-2">Execution Sessions</h3>
          <div className="flex gap-2 mb-3">
            {sessions.map((s) => (
              <button
                key={s.id}
                onClick={() => setSelectedSession(s.id)}
                className={`px-3 py-1 rounded-md text-xs font-medium transition ${
                  selectedSession === s.id
                    ? "bg-gh-blue text-white"
                    : "bg-gh-surface border border-gh-border text-gh-text-secondary hover:text-gh-text"
                }`}
              >
                Attempt #{s.attempt} ({s.status})
              </button>
            ))}
          </div>

          {logs.length > 0 && (
            <div className="p-3 bg-gh-surface border border-gh-border rounded-lg text-sm font-mono max-h-96 overflow-auto">
              {logs.map((log) => (
                <div key={log.id} className="py-0.5">
                  <span className="text-gh-text-muted">
                    {new Date(log.created_at).toLocaleTimeString("ja-JP")}
                  </span>{" "}
                  <span
                    className={
                      log.level === "error"
                        ? "text-gh-red"
                        : log.level === "warn"
                          ? "text-gh-orange"
                          : "text-gh-green"
                    }
                  >
                    [{log.phase}]
                  </span>{" "}
                  <span className="text-gh-text">{log.message}</span>
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* 変更ファイル */}
      {task.changed_files && (
        <div className="mb-6">
          <h3 className="text-sm font-semibold text-gh-text-secondary mb-2">Changed Files</h3>
          <ul className="text-sm font-mono space-y-1">
            {(task.changed_files as string[]).map((f, i) => (
              <li key={i} className="text-gh-text-secondary">
                {f}
              </li>
            ))}
          </ul>
          {task.diff_stats && (
            <pre className="mt-2 text-xs text-gh-text-muted">{task.diff_stats}</pre>
          )}
        </div>
      )}
    </div>
  );
}
