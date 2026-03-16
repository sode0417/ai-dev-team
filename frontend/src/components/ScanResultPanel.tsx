"use client";

import { useEffect, useState, useCallback } from "react";
import { fetchScanResult, approveTask, cancelTask } from "@/lib/api";
import { connectScanWs } from "@/lib/ws";
import type { ScanResult, ScanWsMessage, Task } from "@/types";
import { PriorityBadge } from "@/components/PriorityBadge";

const proposalTypeConfig: Record<string, { label: string; className: string }> = {
  development: { label: "Development", className: "bg-gh-blue/15 text-gh-blue" },
  improvement: { label: "Improvement", className: "bg-gh-orange/15 text-gh-orange" },
  investigation: { label: "Investigation", className: "bg-gh-purple/15 text-gh-purple" },
};

function ProposalTypeBadge({ type: ptype }: { type: string }) {
  const config = proposalTypeConfig[ptype] || {
    label: ptype,
    className: "bg-gh-text-muted/20 text-gh-text-secondary",
  };
  return (
    <span className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${config.className}`}>
      {config.label}
    </span>
  );
}

export function ScanResultPanel({
  scanId,
  onTaskAction,
}: {
  scanId: string;
  onTaskAction?: () => void;
}) {
  const [result, setResult] = useState<ScanResult | null>(null);
  const [progress, setProgress] = useState<ScanWsMessage[]>([]);
  const [loading, setLoading] = useState(true);

  const loadResult = useCallback(() => {
    fetchScanResult(scanId)
      .then((res) => {
        setResult(res.data);
        setLoading(false);
      })
      .catch(() => setLoading(false));
  }, [scanId]);

  useEffect(() => {
    loadResult();

    const ws = connectScanWs(
      scanId,
      (msg) => {
        setProgress((prev) => [...prev, msg]);
        // 完了/エラー時にデータ再取得
        if (msg.phase === "completed" || msg.phase === "error") {
          setTimeout(loadResult, 500);
        }
      },
      () => {
        // 接続切断時にもデータ再取得
        loadResult();
      }
    );

    return () => ws.close();
  }, [scanId, loadResult]);

  const handleApprove = async (taskId: string) => {
    try {
      await approveTask(taskId);
      loadResult();
      onTaskAction?.();
    } catch (e) {
      console.error("Failed to approve task:", e);
    }
  };

  const handleDismiss = async (taskId: string) => {
    try {
      await cancelTask(taskId);
      loadResult();
      onTaskAction?.();
    } catch (e) {
      console.error("Failed to dismiss task:", e);
    }
  };

  // スキャン進行中
  if (loading || !result || result.status === "running") {
    return (
      <div className="rounded-lg border border-gh-border bg-gh-surface p-4">
        <h3 className="text-sm font-semibold mb-3">Scan Progress</h3>
        {progress.length === 0 ? (
          <div className="flex items-center gap-2 text-sm text-gh-text-secondary">
            <span className="inline-block w-4 h-4 border-2 border-gh-blue/40 border-t-gh-blue rounded-full animate-spin" />
            スキャン開始中...
          </div>
        ) : (
          <div className="space-y-1">
            {progress.map((msg, i) => (
              <div key={i} className="flex items-center gap-2 text-sm">
                {msg.phase === "error" ? (
                  <span className="text-gh-red">✗</span>
                ) : msg.phase === "completed" ? (
                  <span className="text-gh-green">✓</span>
                ) : (
                  <span className="inline-block w-3 h-3 border-2 border-gh-blue/40 border-t-gh-blue rounded-full animate-spin" />
                )}
                <span className="text-gh-text-secondary">{msg.message}</span>
              </div>
            ))}
          </div>
        )}
      </div>
    );
  }

  // エラー
  if (result.status === "failed") {
    return (
      <div className="rounded-lg border border-gh-red/30 bg-gh-red/5 p-4">
        <h3 className="text-sm font-semibold text-gh-red mb-2">Scan Failed</h3>
        <p className="text-sm text-gh-text-secondary whitespace-pre-wrap">
          {result.error_log || "Unknown error"}
        </p>
      </div>
    );
  }

  return (
    <div className="rounded-lg border border-gh-border overflow-hidden">
      {/* Header */}
      <div className="px-4 py-3 bg-gh-surface border-b border-gh-border flex items-center justify-between">
        <h3 className="text-sm font-semibold">Scan Results</h3>
        <span className="text-xs text-gh-text-muted">
          {result.completed_at
            ? new Date(result.completed_at).toLocaleString("ja-JP")
            : ""}
        </span>
      </div>

      {/* Analysis */}
      {result.analysis && (
        <div className="px-4 py-3 border-b border-gh-border">
          <div className="flex items-center gap-1.5 mb-1.5">
            <span className="text-sm">📊</span>
            <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider">
              Analysis
            </h4>
          </div>
          <p className="text-sm text-gh-text-secondary whitespace-pre-wrap">
            {result.analysis}
          </p>
        </div>
      )}

      {/* Retrospective */}
      {result.retrospective && (
        <div className="px-4 py-3 border-b border-gh-border">
          <div className="flex items-center gap-1.5 mb-1.5">
            <span className="text-sm">🔄</span>
            <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider">
              Retrospective
            </h4>
          </div>
          <p className="text-sm text-gh-text-secondary whitespace-pre-wrap">
            {result.retrospective}
          </p>
        </div>
      )}

      {/* Priority Actions */}
      {result.priority_actions && result.priority_actions.length > 0 && (
        <div className="px-4 py-3 border-b border-gh-border">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider mb-1.5">
            Priority Actions
          </h4>
          <ul className="space-y-0.5">
            {result.priority_actions.map((action, i) => (
              <li key={i} className="text-sm text-gh-text-secondary flex items-start gap-1.5">
                <span className="text-gh-text-muted mt-0.5">•</span>
                {action}
              </li>
            ))}
          </ul>
        </div>
      )}

      {/* Task Proposals */}
      {result.tasks.length > 0 && (
        <div className="px-4 py-3">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider mb-2">
            Task Proposals ({result.tasks.length})
          </h4>
          <div className="space-y-2">
            {result.tasks.map((task) => (
              <TaskProposalCard
                key={task.id}
                task={task}
                onApprove={() => handleApprove(task.id)}
                onDismiss={() => handleDismiss(task.id)}
              />
            ))}
          </div>
        </div>
      )}

      {/* Improvement Suggestions */}
      {result.improvement_suggestions && result.improvement_suggestions.length > 0 && (
        <div className="px-4 py-3 border-t border-gh-border">
          <h4 className="text-xs font-semibold text-gh-text-secondary uppercase tracking-wider mb-1.5">
            Improvement Suggestions
          </h4>
          <div className="space-y-2">
            {result.improvement_suggestions.map((s, i) => (
              <div key={i} className="text-sm bg-gh-overlay rounded-md p-2.5">
                <div className="flex items-center gap-2 mb-1">
                  <span className="text-xs font-mono px-1.5 py-0.5 rounded bg-gh-orange/15 text-gh-orange">
                    {s.target}
                  </span>
                </div>
                <p className="text-gh-text-secondary">{s.description}</p>
                <p className="text-gh-text-muted text-xs mt-1">理由: {s.reason}</p>
              </div>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}

function TaskProposalCard({
  task,
  onApprove,
  onDismiss,
}: {
  task: Task;
  onApprove: () => void;
  onDismiss: () => void;
}) {
  const isActionable = task.status === "proposed";

  return (
    <div className="rounded-md border border-gh-border bg-gh-overlay p-3">
      <div className="flex items-start justify-between gap-2">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <ProposalTypeBadge type={task.proposal_type} />
            <span className="text-sm font-medium truncate">{task.title}</span>
            <PriorityBadge priority={task.priority} />
          </div>
          <p className="text-xs text-gh-text-secondary line-clamp-2">
            {task.description}
          </p>
        </div>
        {isActionable && (
          <div className="flex gap-1.5 shrink-0">
            <button
              onClick={onApprove}
              className="px-2.5 py-1 text-xs font-medium rounded-md bg-gh-green/15 text-gh-green hover:bg-gh-green/25 transition"
            >
              Approve
            </button>
            <button
              onClick={onDismiss}
              className="px-2.5 py-1 text-xs font-medium rounded-md bg-gh-text-muted/15 text-gh-text-secondary hover:bg-gh-text-muted/25 transition"
            >
              Dismiss
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
