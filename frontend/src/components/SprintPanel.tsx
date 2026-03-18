"use client";

import { useEffect, useState, useCallback } from "react";
import Link from "next/link";
import {
  fetchSprint,
  selectSprintTasks,
  startSprintHearing,
  fetchSprintReadiness,
  createSprintPlan,
  approveSprintPlan,
  submitSprintFeedback,
  cancelSprint,
} from "@/lib/api";
import { connectSprintWs } from "@/lib/ws";
import type { SprintWithTasks, Task, SprintWsMessage } from "@/types";
import { StatusBadge } from "./StatusBadge";
import { PriorityBadge } from "./PriorityBadge";
import { Markdown } from "./Markdown";

const PHASE_LABELS: Record<string, { label: string; color: string }> = {
  selecting: { label: "タスク選定", color: "bg-gh-blue" },
  hearing: { label: "ヒアリング", color: "bg-gh-orange" },
  planning: { label: "実行計画", color: "bg-gh-purple" },
  executing: { label: "実行中", color: "bg-gh-green" },
  retrospective: { label: "振り返り", color: "bg-gh-blue" },
  completed: { label: "完了", color: "bg-gh-text-muted" },
  failed: { label: "失敗", color: "bg-gh-red" },
};

export function SprintPanel({
  sprintId,
  onRefresh,
}: {
  sprintId: string;
  onRefresh?: () => void;
}) {
  const [sprint, setSprint] = useState<SprintWithTasks | null>(null);
  const [wsMessages, setWsMessages] = useState<SprintWsMessage[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [feedback, setFeedback] = useState("");

  const loadSprint = useCallback(() => {
    fetchSprint(sprintId)
      .then((res) => setSprint(res.data))
      .catch((e) => setError(e.message));
  }, [sprintId]);

  useEffect(() => {
    loadSprint();
  }, [loadSprint]);

  // WebSocket
  useEffect(() => {
    const ws = connectSprintWs(
      sprintId,
      (msg) => {
        setWsMessages((prev) => [...prev, msg]);
        // フェーズ変更時にリロード
        if (["completed", "plan_ready", "retrospective", "error", "task_done", "generating_retro"].includes(msg.phase)) {
          loadSprint();
          onRefresh?.();
        }
      },
      () => {}
    );
    return () => ws.close();
  }, [sprintId, loadSprint, onRefresh]);

  // 5秒ポーリング (hearing/executing 時)
  useEffect(() => {
    if (!sprint) return;
    if (!["hearing", "executing", "planning"].includes(sprint.status)) return;
    const id = setInterval(loadSprint, 5000);
    return () => clearInterval(id);
  }, [sprint?.status, loadSprint]);

  if (!sprint) {
    return <div className="text-gh-text-secondary text-sm">読み込み中...</div>;
  }

  const phase = PHASE_LABELS[sprint.status] || { label: sprint.status, color: "bg-gh-text-muted" };
  const activeTasks = sprint.tasks.filter((t) => t.status !== "cancelled");
  const proposedTasks = sprint.tasks.filter((t) => t.status === "proposed");

  return (
    <div className="space-y-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span className={`w-2.5 h-2.5 rounded-full ${phase.color} ${
            ["hearing", "executing", "planning"].includes(sprint.status) ? "animate-pulse" : ""
          }`} />
          <span className="text-sm font-semibold text-gh-text">{phase.label}</span>
          <span className="text-xs text-gh-text-muted">
            {new Date(sprint.created_at).toLocaleString("ja-JP")}
          </span>
        </div>
        <div className="flex items-center gap-2 text-xs text-gh-text-muted">
          <span>{activeTasks.length} tasks</span>
          {sprint.status !== "completed" && sprint.status !== "failed" && (
            <button
              onClick={async () => {
                if (!confirm("このスプリントをキャンセルしますか？進行中のタスクも中止されます。")) return;
                setLoading(true);
                try {
                  await cancelSprint(sprintId);
                  loadSprint();
                  onRefresh?.();
                } catch (e) {
                  setError(e instanceof Error ? e.message : "Failed");
                } finally {
                  setLoading(false);
                }
              }}
              disabled={loading}
              className="px-2 py-0.5 text-gh-red border border-gh-red/30 rounded hover:bg-gh-red/10 transition text-xs disabled:opacity-50"
            >
              キャンセル
            </button>
          )}
        </div>
      </div>

      {error && <div className="text-gh-red text-sm">{error}</div>}

      {/* Phase Timeline */}
      <PhaseTimeline status={sprint.status} />

      {/* Phase-specific content */}
      {sprint.status === "selecting" && (
        <SelectingPhase
          sprint={sprint}
          proposedTasks={proposedTasks}
          loading={loading}
          onSelect={async (approved, rejected) => {
            setLoading(true);
            try {
              await selectSprintTasks(sprintId, approved, rejected);
              loadSprint();
            } catch (e) {
              setError(e instanceof Error ? e.message : "Failed");
            } finally {
              setLoading(false);
            }
          }}
          onStartHearing={async () => {
            setLoading(true);
            try {
              await startSprintHearing(sprintId);
              loadSprint();
            } catch (e) {
              setError(e instanceof Error ? e.message : "Failed");
            } finally {
              setLoading(false);
            }
          }}
        />
      )}

      {sprint.status === "hearing" && (
        <HearingPhase
          sprint={sprint}
          loading={loading}
          onPlan={async () => {
            setLoading(true);
            try {
              await createSprintPlan(sprintId);
              loadSprint();
            } catch (e) {
              setError(e instanceof Error ? e.message : "Failed");
            } finally {
              setLoading(false);
            }
          }}
        />
      )}

      {sprint.status === "planning" && (
        <PlanningPhase
          sprint={sprint}
          loading={loading}
          onApprove={async (maxParallel) => {
            setLoading(true);
            try {
              await approveSprintPlan(sprintId, maxParallel);
              loadSprint();
            } catch (e) {
              setError(e instanceof Error ? e.message : "Failed");
            } finally {
              setLoading(false);
            }
          }}
        />
      )}

      {sprint.status === "executing" && (
        <ExecutingPhase sprint={sprint} wsMessages={wsMessages} />
      )}

      {sprint.status === "retrospective" && (
        <RetrospectivePhase
          sprint={sprint}
          feedback={feedback}
          loading={loading}
          onFeedbackChange={setFeedback}
          onSubmit={async () => {
            setLoading(true);
            try {
              await submitSprintFeedback(sprintId, feedback);
              loadSprint();
              onRefresh?.();
            } catch (e) {
              setError(e instanceof Error ? e.message : "Failed");
            } finally {
              setLoading(false);
            }
          }}
        />
      )}

      {sprint.status === "completed" && (
        <CompletedPhase sprint={sprint} />
      )}

      {sprint.status === "failed" && (
        <div className="p-4 rounded-lg border border-gh-red/30 bg-gh-red/5">
          <p className="text-sm text-gh-red font-medium">スプリント失敗</p>
          {sprint.error_log && (
            <pre className="text-xs text-gh-text-muted mt-2 whitespace-pre-wrap">
              {sprint.error_log}
            </pre>
          )}
        </div>
      )}
    </div>
  );
}

/* ─── Phase Timeline ─── */

const PHASES = ["selecting", "hearing", "planning", "executing", "retrospective", "completed"];

function PhaseTimeline({ status }: { status: string }) {
  const currentIdx = PHASES.indexOf(status);

  return (
    <div className="flex items-center gap-1">
      {PHASES.map((p, i) => {
        const done = i < currentIdx;
        const active = i === currentIdx;
        const label = PHASE_LABELS[p]?.label || p;

        return (
          <div key={p} className="flex items-center gap-1 flex-1">
            <div className="flex flex-col items-center flex-1">
              <div
                className={`w-full h-1 rounded-full ${
                  done ? "bg-gh-green" : active ? "bg-gh-blue" : "bg-gh-border"
                }`}
              />
              <span
                className={`text-[10px] mt-1 ${
                  active ? "text-gh-text font-medium" : "text-gh-text-muted"
                }`}
              >
                {label}
              </span>
            </div>
          </div>
        );
      })}
    </div>
  );
}

/* ─── Selecting Phase ─── */

function SelectingPhase({
  sprint,
  proposedTasks,
  loading,
  onSelect,
  onStartHearing,
}: {
  sprint: SprintWithTasks;
  proposedTasks: Task[];
  loading: boolean;
  onSelect: (approved: string[], rejected: string[]) => void;
  onStartHearing: () => void;
}) {
  const [selections, setSelections] = useState<Record<string, boolean>>({});
  const approvedTasks = sprint.tasks.filter((t) => t.status === "approved");

  // スキャン分析
  if (sprint.scan_analysis) {
    return (
      <div className="space-y-4">
        {/* Analysis */}
        <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase mb-2">
            スキャン分析
          </h4>
          <p className="text-sm text-gh-text">{sprint.scan_analysis}</p>
        </div>

        {/* Priority Actions */}
        {sprint.priority_actions && sprint.priority_actions.length > 0 && (
          <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
            <h4 className="text-xs font-semibold text-gh-text-secondary uppercase mb-2">
              優先アクション
            </h4>
            <ul className="space-y-1">
              {sprint.priority_actions.map((action, i) => (
                <li key={i} className="text-sm text-gh-text flex items-start gap-2">
                  <span className="text-gh-orange shrink-0">•</span>
                  {action}
                </li>
              ))}
            </ul>
          </div>
        )}

        {/* Task Selection */}
        {proposedTasks.length > 0 && (
          <div className="rounded-lg border border-gh-border overflow-hidden">
            <div className="px-4 py-2.5 bg-gh-surface border-b border-gh-border">
              <h4 className="text-xs font-semibold text-gh-text-secondary uppercase">
                タスク選定 ({proposedTasks.length} 件)
              </h4>
            </div>
            {proposedTasks.map((task) => (
              <div key={task.id} className="px-4 py-3 border-b border-gh-border last:border-0 flex items-start gap-3">
                <input
                  type="checkbox"
                  checked={selections[task.id] ?? true}
                  onChange={(e) =>
                    setSelections((prev) => ({ ...prev, [task.id]: e.target.checked }))
                  }
                  className="mt-1 shrink-0"
                />
                <div className="flex-1 min-w-0">
                  <div className="flex items-center gap-2 mb-0.5">
                    <PriorityBadge priority={task.priority} />
                    {task.proposal_type !== "development" && (
                      <span className={`text-[10px] px-1.5 py-0.5 rounded-full font-medium ${
                        task.proposal_type === "improvement"
                          ? "bg-gh-orange/15 text-gh-orange"
                          : task.proposal_type === "operation"
                          ? "bg-gh-green/15 text-gh-green"
                          : "bg-gh-purple/15 text-gh-purple"
                      }`}>
                        {task.proposal_type}
                      </span>
                    )}
                  </div>
                  <p className="text-sm font-medium text-gh-text">{task.title}</p>
                  <p className="text-xs text-gh-text-muted mt-0.5">{task.description}</p>
                </div>
              </div>
            ))}
            <div className="px-4 py-3 bg-gh-surface">
              <button
                onClick={() => {
                  const approved = proposedTasks
                    .filter((t) => selections[t.id] !== false)
                    .map((t) => t.id);
                  const rejected = proposedTasks
                    .filter((t) => selections[t.id] === false)
                    .map((t) => t.id);
                  onSelect(approved, rejected);
                }}
                disabled={loading}
                className="px-3 py-1.5 bg-gh-green/90 text-white rounded-md hover:bg-gh-green transition text-sm font-medium disabled:opacity-50"
              >
                {loading ? "処理中..." : "選定を確定"}
              </button>
            </div>
          </div>
        )}

        {/* Start Hearing */}
        {proposedTasks.length === 0 && approvedTasks.length > 0 && (
          <div className="flex items-center justify-between p-4 rounded-lg border border-gh-border bg-gh-surface">
            <div>
              <p className="text-sm text-gh-text font-medium">
                {approvedTasks.length} 件のタスクが選定済み
              </p>
              <p className="text-xs text-gh-text-muted mt-0.5">
                ヒアリングを開始してタスクの詳細を確認します
              </p>
            </div>
            <button
              onClick={onStartHearing}
              disabled={loading}
              className="px-3 py-1.5 bg-gh-blue/90 text-white rounded-md hover:bg-gh-blue transition text-sm font-medium disabled:opacity-50"
            >
              {loading ? "開始中..." : "ヒアリング開始"}
            </button>
          </div>
        )}
      </div>
    );
  }

  // スキャン中
  return (
    <div className="flex items-center gap-3 p-4 rounded-lg border border-gh-border bg-gh-surface">
      <div className="w-5 h-5 border-2 border-gh-blue border-t-transparent rounded-full animate-spin" />
      <p className="text-sm text-gh-text-secondary">スキャン中...</p>
    </div>
  );
}

/* ─── Hearing Phase ─── */

function HearingPhase({
  sprint,
  loading,
  onPlan,
}: {
  sprint: SprintWithTasks;
  loading: boolean;
  onPlan: () => void;
}) {
  // バックエンドの all_tasks_ready と一致: failed/completed タスクは readiness をブロックしない
  const activeTasks = sprint.tasks.filter((t) =>
    !["cancelled", "proposed", "completed", "failed"].includes(t.status)
  );
  const readyTasks = activeTasks.filter((t) => t.status === "awaiting_approval");
  const allReady = readyTasks.length === activeTasks.length && activeTasks.length > 0;

  return (
    <div className="space-y-4">
      <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
        <div className="flex items-center justify-between mb-3">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase">
            ヒアリング進捗 ({readyTasks.length}/{activeTasks.length})
          </h4>
          {allReady && (
            <button
              onClick={onPlan}
              disabled={loading}
              className="px-3 py-1.5 bg-gh-purple/90 text-white rounded-md hover:bg-gh-purple transition text-sm font-medium disabled:opacity-50"
            >
              {loading ? "計画中..." : "実行計画を作成"}
            </button>
          )}
        </div>

        <div className="space-y-2">
          {activeTasks.map((task) => (
            <div key={task.id} className="flex items-center gap-3 text-sm">
              <StatusBadge status={task.status} />
              <Link href={`/tasks/${task.id}`} className="text-gh-text hover:text-gh-link transition flex-1 truncate">
                {task.title}
              </Link>
              {task.status === "hearing" && (
                <span className="text-xs text-gh-orange">回答待ち</span>
              )}
              {task.status === "awaiting_approval" && (
                <span className="text-xs text-gh-green">準備完了</span>
              )}
            </div>
          ))}
        </div>
      </div>

      {!allReady && (
        <p className="text-xs text-gh-text-muted">
          各タスクの詳細ページでヒアリングに回答してください。全タスクが準備完了になると計画フェーズに進めます。
        </p>
      )}
    </div>
  );
}

/* ─── Planning Phase ─── */

function PlanningPhase({
  sprint,
  loading,
  onApprove,
}: {
  sprint: SprintWithTasks;
  loading: boolean;
  onApprove: (maxParallel: number) => void;
}) {
  const [maxParallel, setMaxParallel] = useState(3);

  if (!sprint.execution_plan) {
    return (
      <div className="flex items-center gap-3 p-4 rounded-lg border border-gh-border bg-gh-surface">
        <div className="w-5 h-5 border-2 border-gh-purple border-t-transparent rounded-full animate-spin" />
        <p className="text-sm text-gh-text-secondary">PM Agent が実行計画を作成中...</p>
      </div>
    );
  }

  // execution_group ごとのタスク数を集計
  const groupCounts: Record<number, number> = {};
  sprint.tasks
    .filter((t) => t.status !== "cancelled" && t.status !== "proposed")
    .forEach((t) => {
      groupCounts[t.execution_group] = (groupCounts[t.execution_group] || 0) + 1;
    });
  const groupEntries = Object.entries(groupCounts)
    .map(([g, c]) => ({ group: Number(g), count: c }))
    .sort((a, b) => a.group - b.group);

  return (
    <div className="space-y-4">
      <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
        <h4 className="text-xs font-semibold text-gh-text-secondary uppercase mb-3">
          実行計画
        </h4>
        <div className="prose-sm">
          <Markdown>{sprint.execution_plan}</Markdown>
        </div>
      </div>

      {/* 並列グループプレビュー */}
      {groupEntries.length > 1 && (
        <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase mb-2">
            並列実行グループ
          </h4>
          <div className="flex gap-2 flex-wrap">
            {groupEntries.map(({ group, count }) => (
              <span
                key={group}
                className="text-xs px-2 py-1 rounded-full bg-gh-blue/10 text-gh-blue"
              >
                Group {group}: {count}タスク
              </span>
            ))}
          </div>
        </div>
      )}

      <div className="flex items-center gap-4">
        <button
          onClick={() => onApprove(maxParallel)}
          disabled={loading}
          className="px-4 py-2 bg-gh-green/90 text-white rounded-md hover:bg-gh-green transition text-sm font-medium disabled:opacity-50"
        >
          {loading ? "開始中..." : "承認して実行開始"}
        </button>
        <label className="flex items-center gap-2 text-xs text-gh-text-muted">
          最大並列数:
          <select
            value={maxParallel}
            onChange={(e) => setMaxParallel(Number(e.target.value))}
            className="px-2 py-1 bg-gh-canvas border border-gh-border rounded text-sm text-gh-text"
          >
            {[1, 2, 3, 4, 5].map((n) => (
              <option key={n} value={n}>
                {n}
              </option>
            ))}
          </select>
        </label>
      </div>
    </div>
  );
}

/* ─── Executing Phase ─── */

function ExecutingPhase({
  sprint,
  wsMessages,
}: {
  sprint: SprintWithTasks;
  wsMessages: SprintWsMessage[];
}) {
  const activeTasks = sprint.tasks.filter((t) => t.status !== "cancelled" && t.status !== "proposed");

  // グループごとにタスクを分類
  const groups: Record<number, Task[]> = {};
  activeTasks.forEach((t) => {
    const g = t.execution_group;
    if (!groups[g]) groups[g] = [];
    groups[g].push(t);
  });
  const groupEntries = Object.entries(groups)
    .map(([g, tasks]) => ({ group: Number(g), tasks }))
    .sort((a, b) => a.group - b.group);

  const hasMultipleGroups = groupEntries.length > 1;

  return (
    <div className="space-y-4">
      {/* Task progress by group */}
      <div className="rounded-lg border border-gh-border overflow-hidden">
        <div className="px-4 py-2.5 bg-gh-surface border-b border-gh-border flex items-center justify-between">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase">
            実行進捗
          </h4>
          {sprint.max_parallel_tasks > 1 && (
            <span className="text-[10px] text-gh-text-muted">
              最大 {sprint.max_parallel_tasks} 並列
            </span>
          )}
        </div>
        {groupEntries.map(({ group, tasks }) => (
          <div key={group}>
            {hasMultipleGroups && (
              <div className="px-4 py-1.5 bg-gh-canvas border-b border-gh-border">
                <span className="text-[10px] font-medium text-gh-text-muted uppercase">
                  Group {group}
                  {tasks.length > 1 && " (並列)"}
                </span>
              </div>
            )}
            {tasks.map((task) => (
              <div key={task.id} className="px-4 py-3 border-b border-gh-border last:border-0 flex items-center gap-3">
                <StatusBadge status={task.status} />
                <Link href={`/tasks/${task.id}`} className="text-sm text-gh-text hover:text-gh-link transition flex-1 truncate">
                  {task.title}
                </Link>
                {task.pr_url && (
                  <a href={task.pr_url} target="_blank" rel="noopener noreferrer"
                    className="text-xs text-gh-link hover:underline shrink-0">
                    PR
                  </a>
                )}
              </div>
            ))}
          </div>
        ))}
      </div>

      {/* Live messages */}
      {wsMessages.length > 0 && (
        <div className="p-3 rounded-lg border border-gh-border bg-gh-surface max-h-48 overflow-y-auto">
          {wsMessages.map((msg, i) => (
            <div key={i} className="text-xs text-gh-text-secondary py-0.5">
              <span className="text-gh-text-muted">[{msg.phase}]</span> {msg.message}
            </div>
          ))}
        </div>
      )}
    </div>
  );
}

/* ─── Retrospective Phase ─── */

function RetrospectivePhase({
  sprint,
  feedback,
  loading,
  onFeedbackChange,
  onSubmit,
}: {
  sprint: SprintWithTasks;
  feedback: string;
  loading: boolean;
  onFeedbackChange: (v: string) => void;
  onSubmit: () => void;
}) {
  const completed = sprint.tasks.filter((t) => t.status === "completed");
  const failed = sprint.tasks.filter((t) => t.status === "failed");

  return (
    <div className="space-y-4">
      {/* Results summary */}
      <div className="flex gap-3">
        <div className="flex-1 p-3 rounded-lg border border-gh-green/30 bg-gh-green/5 text-center">
          <p className="text-2xl font-bold text-gh-green">{completed.length}</p>
          <p className="text-xs text-gh-text-muted">成功</p>
        </div>
        <div className="flex-1 p-3 rounded-lg border border-gh-red/30 bg-gh-red/5 text-center">
          <p className="text-2xl font-bold text-gh-red">{failed.length}</p>
          <p className="text-xs text-gh-text-muted">失敗</p>
        </div>
      </div>

      {/* Retrospective */}
      {sprint.retrospective && (
        <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase mb-3">
            振り返り (PM Agent)
          </h4>
          <div className="prose-sm">
            <Markdown>{sprint.retrospective}</Markdown>
          </div>
        </div>
      )}

      {/* Feedback form */}
      <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
        <h4 className="text-xs font-semibold text-gh-text-secondary uppercase mb-2">
          フィードバック
        </h4>
        <p className="text-xs text-gh-text-muted mb-3">
          次のスプリントに活かすフィードバックを入力してください
        </p>
        <textarea
          value={feedback}
          onChange={(e) => onFeedbackChange(e.target.value)}
          placeholder="良かった点、改善してほしい点、次に取り組みたいことなど..."
          rows={4}
          className="w-full px-3 py-2 bg-gh-canvas border border-gh-border rounded-md text-sm text-gh-text placeholder:text-gh-text-muted focus:outline-none focus:border-gh-blue focus:ring-1 focus:ring-gh-blue/40 resize-none"
        />
        <button
          onClick={onSubmit}
          disabled={loading || !feedback.trim()}
          className="mt-2 px-4 py-2 bg-gh-blue/90 text-white rounded-md hover:bg-gh-blue transition text-sm font-medium disabled:opacity-50"
        >
          {loading ? "送信中..." : "送信してスプリント完了"}
        </button>
      </div>
    </div>
  );
}

/* ─── Completed Phase ─── */

function CompletedPhase({ sprint }: { sprint: SprintWithTasks }) {
  const completed = sprint.tasks.filter((t) => t.status === "completed");
  const failed = sprint.tasks.filter((t) => t.status === "failed");

  return (
    <div className="space-y-4">
      <div className="flex gap-3">
        <div className="flex-1 p-3 rounded-lg border border-gh-green/30 bg-gh-green/5 text-center">
          <p className="text-2xl font-bold text-gh-green">{completed.length}</p>
          <p className="text-xs text-gh-text-muted">成功</p>
        </div>
        {failed.length > 0 && (
          <div className="flex-1 p-3 rounded-lg border border-gh-red/30 bg-gh-red/5 text-center">
            <p className="text-2xl font-bold text-gh-red">{failed.length}</p>
            <p className="text-xs text-gh-text-muted">失敗</p>
          </div>
        )}
      </div>

      {sprint.user_feedback && (
        <div className="p-4 rounded-lg border border-gh-border bg-gh-surface">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase mb-2">
            ユーザーフィードバック
          </h4>
          <p className="text-sm text-gh-text whitespace-pre-wrap">{sprint.user_feedback}</p>
        </div>
      )}

      {/* Task list */}
      <div className="rounded-lg border border-gh-border overflow-hidden">
        {sprint.tasks
          .filter((t) => t.status !== "cancelled")
          .map((task) => (
            <div key={task.id} className="px-4 py-2.5 border-b border-gh-border last:border-0 flex items-center gap-3">
              <StatusBadge status={task.status} />
              <Link href={`/tasks/${task.id}`} className="text-sm text-gh-text hover:text-gh-link transition flex-1 truncate">
                {task.title}
              </Link>
              {task.pr_url && (
                <a href={task.pr_url} target="_blank" rel="noopener noreferrer"
                  className="text-xs text-gh-link hover:underline shrink-0">
                  PR
                </a>
              )}
            </div>
          ))}
      </div>
    </div>
  );
}
