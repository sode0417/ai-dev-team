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

  // WebSocket 接続
  useEffect(() => {
    if (!task) return;
    const isActive = ["planning", "executing", "reviewing"].includes(task.status);
    if (!isActive) return;

    wsRef.current = connectTaskWs(
      id,
      (msg) => {
        setWsMessages((prev) => [...prev, msg]);
      },
      () => {
        // 接続切断時にリロード
        load();
      }
    );

    return () => {
      wsRef.current?.close();
    };
  }, [task?.status]);

  // セッションのログを取得
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
    return <div className="text-red-500">{error}</div>;
  }

  if (!task) {
    return <div className="text-slate-500">読み込み中...</div>;
  }

  return (
    <div className="max-w-4xl">
      <div className="flex items-center gap-3 mb-4">
        <StatusBadge status={task.status} />
        <PriorityBadge priority={task.priority} />
        <h2 className="text-2xl font-bold">{task.title}</h2>
      </div>

      {error && <div className="text-red-500 mb-4 text-sm">{error}</div>}

      <div className="mb-6 p-4 rounded border border-slate-200 dark:border-slate-700">
        <p className="whitespace-pre-wrap">{task.description}</p>
        <div className="mt-3 text-sm text-slate-500 space-y-1">
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
                className="text-blue-500 underline"
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
              className="px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 text-sm"
            >
              Approve
            </button>
            <button
              onClick={() => handleAction("execute")}
              className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 text-sm"
            >
              Execute Now
            </button>
          </>
        )}
        {task.status === "approved" && (
          <button
            onClick={() => handleAction("execute")}
            className="px-4 py-2 bg-green-600 text-white rounded hover:bg-green-700 text-sm"
          >
            Execute
          </button>
        )}
        {!["completed", "failed", "cancelled"].includes(task.status) && (
          <button
            onClick={() => handleAction("cancel")}
            className="px-4 py-2 bg-red-600 text-white rounded hover:bg-red-700 text-sm"
          >
            Cancel
          </button>
        )}
      </div>

      {/* 実行計画 */}
      {task.plan && (
        <div className="mb-6">
          <h3 className="text-lg font-semibold mb-2">Plan</h3>
          <pre className="p-3 bg-slate-100 dark:bg-slate-800 rounded text-sm whitespace-pre-wrap overflow-auto max-h-96">
            {task.plan}
          </pre>
        </div>
      )}

      {/* エラーログ */}
      {task.error_log && (
        <div className="mb-6">
          <h3 className="text-lg font-semibold mb-2 text-red-600">Error</h3>
          <pre className="p-3 bg-red-50 dark:bg-red-900/20 rounded text-sm whitespace-pre-wrap text-red-700 dark:text-red-400">
            {task.error_log}
          </pre>
        </div>
      )}

      {/* リアルタイム進捗 (WebSocket) */}
      {wsMessages.length > 0 && (
        <div className="mb-6">
          <h3 className="text-lg font-semibold mb-2">Live Progress</h3>
          <div className="p-3 bg-slate-50 dark:bg-slate-800 rounded space-y-1 max-h-48 overflow-auto">
            {wsMessages.map((msg, i) => (
              <div key={i} className="text-sm">
                <span className="font-mono text-slate-500">[{msg.phase}]</span>{" "}
                {msg.message}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* 実行セッション */}
      {sessions.length > 0 && (
        <div className="mb-6">
          <h3 className="text-lg font-semibold mb-2">Execution Sessions</h3>
          <div className="flex gap-2 mb-3">
            {sessions.map((s) => (
              <button
                key={s.id}
                onClick={() => setSelectedSession(s.id)}
                className={`px-3 py-1 rounded text-sm ${
                  selectedSession === s.id
                    ? "bg-blue-600 text-white"
                    : "bg-slate-200 dark:bg-slate-700"
                }`}
              >
                Attempt #{s.attempt} ({s.status})
              </button>
            ))}
          </div>

          {/* 実行ログ */}
          {logs.length > 0 && (
            <div className="p-3 bg-slate-900 rounded text-sm font-mono text-slate-200 max-h-96 overflow-auto">
              {logs.map((log) => (
                <div key={log.id} className="py-0.5">
                  <span className="text-slate-500">
                    {new Date(log.created_at).toLocaleTimeString("ja-JP")}
                  </span>{" "}
                  <span
                    className={
                      log.level === "error"
                        ? "text-red-400"
                        : log.level === "warn"
                          ? "text-yellow-400"
                          : "text-green-400"
                    }
                  >
                    [{log.phase}]
                  </span>{" "}
                  {log.message}
                </div>
              ))}
            </div>
          )}
        </div>
      )}

      {/* 変更ファイル */}
      {task.changed_files && (
        <div className="mb-6">
          <h3 className="text-lg font-semibold mb-2">Changed Files</h3>
          <ul className="text-sm font-mono space-y-1">
            {(task.changed_files as string[]).map((f, i) => (
              <li key={i} className="text-slate-600 dark:text-slate-400">
                {f}
              </li>
            ))}
          </ul>
          {task.diff_stats && (
            <pre className="mt-2 text-xs text-slate-500">{task.diff_stats}</pre>
          )}
        </div>
      )}
    </div>
  );
}
