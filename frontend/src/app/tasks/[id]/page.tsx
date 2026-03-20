"use client";

import { useEffect, useState, useRef, useCallback, use } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import {
  fetchTask,
  fetchExecutions,
  fetchExecutionLogs,
  fetchHearings,
  approveTask,
  executeTask,
  cancelTask,
  API_BASE,
} from "@/lib/api";
import { connectTaskWs } from "@/lib/ws";
import type { Task, ExecutionSession, ExecutionLog, WsMessage, TaskHearing } from "@/types";
import { StatusBadge } from "@/components/StatusBadge";
import { PriorityBadge } from "@/components/PriorityBadge";
import { HearingPanel } from "@/components/HearingPanel";
import { PlanApprovalPanel } from "@/components/PlanApprovalPanel";
import { Markdown } from "@/components/Markdown";

/* ─── Phase definitions ─── */

const PHASE_ORDER = ["hearing", "planning", "awaiting_approval", "executing", "reviewing", "qa", "completed", "failed"] as const;

type PhaseName = (typeof PHASE_ORDER)[number];

const PHASE_META: Record<string, { icon: string; label: string; color: string }> = {
  hearing:            { icon: "💬", label: "ヒアリング",   color: "gh-orange" },
  planning:           { icon: "📋", label: "計画",         color: "gh-orange" },
  awaiting_approval:  { icon: "✋", label: "承認待ち",     color: "gh-orange" },
  executing:          { icon: "⚡", label: "実行",         color: "gh-blue" },
  reviewing:          { icon: "🔍", label: "レビュー",     color: "gh-purple" },
  qa:                 { icon: "🧪", label: "QA",           color: "gh-purple" },
  completed:          { icon: "✅", label: "完了",         color: "gh-green" },
  failed:             { icon: "❌", label: "失敗",         color: "gh-red" },
};

function phaseIndex(phase: string): number {
  const idx = PHASE_ORDER.indexOf(phase as PhaseName);
  return idx >= 0 ? idx : -1;
}

/* ─── Main page ─── */

export default function TaskDetailPage({
  params,
}: {
  params: Promise<{ id: string }>;
}) {
  const { id } = use(params);
  const router = useRouter();
  const [task, setTask] = useState<Task | null>(null);
  const [sessions, setSessions] = useState<ExecutionSession[]>([]);
  const [logs, setLogs] = useState<ExecutionLog[]>([]);
  const [wsMessages, setWsMessages] = useState<WsMessage[]>([]);
  const [hearings, setHearings] = useState<TaskHearing[]>([]);
  const [error, setError] = useState<string | null>(null);
  const wsRef = useRef<WebSocket | null>(null);
  const pollRef = useRef<ReturnType<typeof setInterval> | null>(null);

  const load = useCallback(async () => {
    try {
      const [taskRes, execRes, hearingRes] = await Promise.all([
        fetchTask(id),
        fetchExecutions(id),
        fetchHearings(id),
      ]);
      setTask(taskRes.data);
      setSessions(execRes.data);
      setHearings(hearingRes.data);
      // 最新セッションのログ取得
      if (execRes.data.length > 0) {
        const latestSession = execRes.data[0];
        const logRes = await fetchExecutionLogs(latestSession.id);
        setLogs(logRes.data);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : "Failed to load");
    }
  }, [id]);

  // 初回ロード
  useEffect(() => {
    load();
  }, [load]);

  // WebSocket 接続 + ポーリング
  useEffect(() => {
    if (!task) return;
    const isActive = ["hearing", "planning", "awaiting_approval", "executing", "reviewing"].includes(task.status);
    if (!isActive) return;

    // WebSocket でリアルタイム進捗
    wsRef.current = connectTaskWs(
      id,
      (msg) => {
        setWsMessages((prev) => [...prev, msg]);
        // phase 変化時にデータ再取得
        load();
      },
      () => load()
    );

    // ポーリングで状態同期（WS メッセージがないフェーズ遷移をカバー）
    pollRef.current = setInterval(load, 5000);

    return () => {
      wsRef.current?.close();
      if (pollRef.current) clearInterval(pollRef.current);
    };
  }, [task?.status, id, load]);

  const handleAction = async (action: "approve" | "execute" | "execute-skip" | "cancel") => {
    try {
      if (action === "approve") await approveTask(id);
      else if (action === "execute") await executeTask(id, false);
      else if (action === "execute-skip") await executeTask(id, true);
      else if (action === "cancel") await cancelTask(id);
      load();
    } catch (e) {
      setError(e instanceof Error ? e.message : "Action failed");
    }
  };

  if (error && !task) return <div className="text-gh-red">{error}</div>;
  if (!task) return <div className="text-gh-text-secondary">読み込み中...</div>;

  const currentPhase = task.status;
  const isTerminal = ["completed", "failed", "cancelled"].includes(currentPhase);

  return (
    <div className="max-w-4xl">
      {/* ─── Back + Header ─── */}
      <button
        onClick={() => router.back()}
        className="flex items-center gap-1 text-sm text-gh-text-muted hover:text-gh-text transition mb-3 cursor-pointer"
      >
        <svg className="w-4 h-4" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
          <path strokeLinecap="round" strokeLinejoin="round" d="M15 19l-7-7 7-7" />
        </svg>
        戻る
      </button>
      <div className="flex items-center gap-3 mb-1">
        <StatusBadge status={task.status} />
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
        <h2 className="text-xl font-semibold">{task.title}</h2>
      </div>

      <div className="mb-5 pl-1">
        <p className="text-sm text-gh-text-secondary mb-1">{task.description}</p>
        <div className="text-xs text-gh-text-muted space-x-3">
          <span>作成: {new Date(task.created_at).toLocaleString("ja-JP")}</span>
          {task.started_at && <span>開始: {new Date(task.started_at).toLocaleString("ja-JP")}</span>}
          {task.completed_at && <span>完了: {new Date(task.completed_at).toLocaleString("ja-JP")}</span>}
        </div>
        {task.sprint_id && (
          <Link
            href={`/projects/${task.project_id}?tab=sprint`}
            className="inline-flex items-center gap-1 text-xs text-gh-purple hover:underline mt-1"
          >
            <span className="w-1.5 h-1.5 rounded-full bg-gh-purple inline-block" />
            Sprint
          </Link>
        )}
      </div>

      {/* ─── Definition of Done ─── */}
      {task.definition_of_done && (
        <div className="mb-5 rounded-lg border border-gh-border bg-gh-surface/50 overflow-hidden">
          <button
            onClick={() => {
              const el = document.getElementById("dod-body");
              if (el) el.classList.toggle("hidden");
            }}
            className="w-full flex items-center justify-between px-4 py-2 text-sm font-medium text-gh-text-secondary hover:bg-gh-overlay transition cursor-pointer"
          >
            <span>完了条件 (Definition of Done)</span>
            <svg className="w-4 h-4 text-gh-text-muted" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={2}>
              <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
            </svg>
          </button>
          <div id="dod-body" className="px-4 py-3 border-t border-gh-border/50">
            <DoDChecklist dod={task.definition_of_done} />
          </div>
        </div>
      )}

      {error && <div className="text-gh-red mb-4 text-sm">{error}</div>}

      {/* ─── Action Buttons ─── */}
      {!isTerminal && (
        <div className="flex gap-2 mb-6">
          {task.status === "proposed" && (
            <>
              <ActionButton onClick={() => handleAction("approve")} variant="blue">Approve</ActionButton>
              <ActionButton onClick={() => handleAction("execute")} variant="green">Execute</ActionButton>
              <ActionButton onClick={() => handleAction("execute-skip")} variant="outline-green" title="ヒアリング・計画承認をスキップして即時実行">即時実行</ActionButton>
            </>
          )}
          {task.status === "approved" && (
            <>
              <ActionButton onClick={() => handleAction("execute")} variant="green">Execute</ActionButton>
              <ActionButton onClick={() => handleAction("execute-skip")} variant="outline-green" title="ヒアリング・計画承認をスキップして即時実行">即時実行</ActionButton>
            </>
          )}
          <ActionButton onClick={() => handleAction("cancel")} variant="outline-red">Cancel</ActionButton>
        </div>
      )}

      {/* ─── Phase Cards (Timeline) ─── */}
      <div className="space-y-3">
        {/* Hearing Card */}
        <PhaseCard
          phase="hearing"
          currentPhase={currentPhase}
          show={hearings.length > 0 || currentPhase === "hearing"}
          collapsible={currentPhase !== "hearing" && hearings.length > 0}
          defaultCollapsed={currentPhase !== "hearing"}
        >
          {currentPhase === "hearing" && hearings.length > 0 ? (
            <HearingPanel taskId={id} hearings={hearings} onAnswered={load} />
          ) : hearings.length > 0 ? (
            <AnsweredHearings hearings={hearings} />
          ) : null}
        </PhaseCard>

        {/* Planning Card */}
        <PhaseCard
          phase="planning"
          currentPhase={currentPhase}
          show={phaseIndex(currentPhase) >= phaseIndex("planning") || !!task.plan}
        >
          {currentPhase === "planning" && (
            <div className="flex items-center gap-2 text-sm text-gh-text-secondary">
              <Spinner /> Planner Agent 分析中...
            </div>
          )}
        </PhaseCard>

        {/* Awaiting Approval Card */}
        <PhaseCard
          phase="awaiting_approval"
          currentPhase={currentPhase}
          show={currentPhase === "awaiting_approval" || (!!task.plan && phaseIndex(currentPhase) > phaseIndex("awaiting_approval"))}
        >
          {currentPhase === "awaiting_approval" && task.plan ? (
            <PlanApprovalPanel taskId={id} plan={task.plan} onAction={load} />
          ) : task.plan && phaseIndex(currentPhase) > phaseIndex("awaiting_approval") ? (
            <div className="text-xs text-gh-text-muted">計画承認済み</div>
          ) : null}
        </PhaseCard>

        {/* Execution Card */}
        <PhaseCard
          phase="executing"
          currentPhase={currentPhase}
          show={phaseIndex(currentPhase) >= phaseIndex("executing")}
        >
          {(currentPhase === "executing" || currentPhase === "reviewing") && (
            <LiveProgress messages={wsMessages} logs={logs} />
          )}
        </PhaseCard>

        {/* Review Card */}
        {sessions.length > 0 && sessions[0].review_output && (
          <PhaseCard
            phase="reviewing"
            currentPhase={currentPhase}
            show={true}
          >
            <div className="text-sm">
              <span className={`font-medium ${sessions[0].review_verdict === "APPROVE" ? "text-gh-green" : "text-gh-orange"}`}>
                Verdict: {sessions[0].review_verdict}
              </span>
            </div>
          </PhaseCard>
        )}

        {/* QA Card */}
        {sessions.length > 0 && sessions[0].qa_output && (
          <PhaseCard
            phase="qa"
            currentPhase={currentPhase}
            show={true}
          >
            <div className="space-y-3">
              <div className="text-sm">
                <span className={`font-medium ${sessions[0].qa_passed ? "text-gh-green" : "text-gh-orange"}`}>
                  QA Verdict: {sessions[0].qa_passed ? "PASS" : "FAIL"}
                </span>
              </div>
              {sessions[0].qa_screenshots && sessions[0].qa_screenshots.length > 0 && (
                <div className="space-y-2">
                  <div className="text-xs text-gh-text-muted font-medium">スクリーンショット</div>
                  <div className="grid grid-cols-2 gap-2">
                    {sessions[0].qa_screenshots.map((filename, i) => (
                      <a
                        key={i}
                        href={`${API_BASE}/api/tasks/${task.id}/screenshots/${filename}`}
                        target="_blank"
                        rel="noopener noreferrer"
                        className="block rounded border border-gh-border overflow-hidden hover:border-gh-blue/50 transition"
                      >
                        <img
                          src={`${API_BASE}/api/tasks/${task.id}/screenshots/${filename}`}
                          alt={filename}
                          className="w-full h-auto"
                        />
                        <div className="px-2 py-1 text-[10px] text-gh-text-muted bg-gh-surface truncate">
                          {filename}
                        </div>
                      </a>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </PhaseCard>
        )}

        {/* Completed Card */}
        <PhaseCard
          phase={task.status === "failed" ? "failed" : "completed"}
          currentPhase={currentPhase}
          show={isTerminal}
        >
          {task.status === "completed" && (
            <CompletedCard task={task} />
          )}
          {task.status === "failed" && task.error_log && (
            <div className="text-sm text-gh-red whitespace-pre-wrap font-mono text-xs">
              {task.error_log}
            </div>
          )}
          {task.status === "cancelled" && (
            <div className="text-sm text-gh-text-muted">タスクはキャンセルされました</div>
          )}
        </PhaseCard>
      </div>

      {/* ─── Plan (collapsible, shown after planning) ─── */}
      {task.plan && currentPhase !== "awaiting_approval" && (
        <Accordion title="計画内容" defaultOpen={isTerminal} className="mt-4">
          <div className="max-h-[600px] overflow-auto">
            <Markdown>{task.plan}</Markdown>
          </div>
        </Accordion>
      )}

      {/* ─── Execution Logs ─── */}
      {sessions.length > 0 && logs.length > 0 && (
        <Accordion title={`実行ログ — Attempt #${sessions[0].attempt}`} defaultOpen={!isTerminal} className="mt-3">
          <div className="text-sm font-mono max-h-80 overflow-auto space-y-0.5">
            {logs.map((log) => (
              <div key={log.id} className="py-0.5">
                <span className="text-gh-text-muted">
                  {new Date(log.created_at).toLocaleTimeString("ja-JP")}
                </span>{" "}
                <span className={
                  log.level === "error" ? "text-gh-red"
                    : log.level === "warn" ? "text-gh-orange"
                    : "text-gh-green"
                }>
                  [{log.phase}]
                </span>{" "}
                <span className="text-gh-text">{log.message}</span>
              </div>
            ))}
          </div>
        </Accordion>
      )}

      {/* ─── Changed Files ─── */}
      {task.changed_files && (
        <Accordion title="変更ファイル" defaultOpen={true} className="mt-3">
          <ul className="text-sm font-mono space-y-0.5">
            {(task.changed_files as string[]).map((f, i) => (
              <li key={i} className="text-gh-text-secondary">{f}</li>
            ))}
          </ul>
          {task.diff_stats && (
            <pre className="mt-2 text-xs text-gh-text-muted">{task.diff_stats}</pre>
          )}
        </Accordion>
      )}
    </div>
  );
}

/* ─── Phase Card ─── */

const PHASE_ACTIVE_STYLES: Record<string, { border: string; bg: string; shadow: string; text: string; badge: string; dot: string; dotPing: string }> = {
  "gh-orange":     { border: "border-gh-orange/50",  bg: "bg-gh-orange/5",  shadow: "shadow-gh-orange/20", text: "text-gh-orange",     badge: "bg-gh-orange/15 text-gh-orange",     dot: "bg-gh-orange",     dotPing: "bg-gh-orange/60" },
  "gh-blue":       { border: "border-gh-blue/50",    bg: "bg-gh-blue/5",    shadow: "shadow-gh-blue/20",   text: "text-gh-blue",       badge: "bg-gh-blue/15 text-gh-blue",         dot: "bg-gh-blue",       dotPing: "bg-gh-blue/60" },
  "gh-purple":     { border: "border-gh-purple/50",  bg: "bg-gh-purple/5",  shadow: "shadow-gh-purple/20", text: "text-gh-purple",     badge: "bg-gh-purple/15 text-gh-purple",     dot: "bg-gh-purple",     dotPing: "bg-gh-purple/60" },
  "gh-green":      { border: "border-gh-green/50",   bg: "bg-gh-green/5",   shadow: "shadow-gh-green/20",  text: "text-gh-green",      badge: "bg-gh-green/15 text-gh-green",       dot: "bg-gh-green",      dotPing: "bg-gh-green/60" },
  "gh-red":        { border: "border-gh-red/50",     bg: "bg-gh-red/5",     shadow: "shadow-gh-red/20",    text: "text-gh-red",        badge: "bg-gh-red/15 text-gh-red",           dot: "bg-gh-red",        dotPing: "bg-gh-red/60" },
  "gh-text-muted": { border: "border-gh-border",     bg: "bg-gh-surface/30",shadow: "",                    text: "text-gh-text-muted", badge: "bg-gh-text-muted/15 text-gh-text-muted", dot: "bg-gh-text-muted", dotPing: "bg-gh-text-muted/60" },
};

function PhaseCard({
  phase,
  currentPhase,
  show,
  children,
  collapsible = false,
  defaultCollapsed = false,
}: {
  phase: string;
  currentPhase: string;
  show: boolean;
  children: React.ReactNode;
  collapsible?: boolean;
  defaultCollapsed?: boolean;
}) {
  const [collapsed, setCollapsed] = useState(defaultCollapsed);

  if (!show) return null;

  const meta = PHASE_META[phase] || { icon: "•", label: phase, color: "gh-text-muted" };
  const styles = PHASE_ACTIVE_STYLES[meta.color] || PHASE_ACTIVE_STYLES["gh-text-muted"];
  const isTerminalPhase = phase === "completed" || phase === "failed";
  const isActive = !isTerminalPhase && currentPhase === phase;
  const isDone = isTerminalPhase
    ? currentPhase === phase
    : phaseIndex(currentPhase) > phaseIndex(phase);

  const showBody = children && (!collapsible || !collapsed);

  return (
    <div
      className={`relative rounded-lg border transition-all ${
        isActive
          ? `${styles.border} ${styles.bg} shadow-[0_0_12px_-3px] ${styles.shadow}`
          : isTerminalPhase && isDone
          ? `${styles.border} ${styles.bg}`
          : isDone
          ? "border-gh-border bg-gh-surface/50 opacity-80"
          : "border-gh-border bg-gh-surface/30"
      }`}
    >
      {/* Card header */}
      <div
        className={`flex items-center gap-2 px-4 py-2.5 ${showBody ? "border-b border-gh-border/50" : ""} ${collapsible ? "cursor-pointer hover:bg-gh-overlay/30 transition" : ""}`}
        onClick={collapsible ? () => setCollapsed(!collapsed) : undefined}
      >
        {isActive && (
          <span className="relative flex h-2.5 w-2.5">
            <span className={`animate-ping absolute inline-flex h-full w-full rounded-full ${styles.dotPing} opacity-75`} />
            <span className={`relative inline-flex rounded-full h-2.5 w-2.5 ${styles.dot}`} />
          </span>
        )}
        {!isActive && isDone && <span className="text-gh-green text-xs">✓</span>}
        <span className="text-sm">{meta.icon}</span>
        <span className={`text-sm font-medium ${isActive ? styles.text : "text-gh-text-secondary"}`}>
          {meta.label}
        </span>
        {isActive && (
          <span className={`ml-auto text-[10px] px-1.5 py-0.5 rounded-full ${styles.badge} font-medium`}>
            進行中
          </span>
        )}
        {collapsible && (
          <svg
            className={`w-4 h-4 ml-auto text-gh-text-muted transition-transform ${collapsed ? "" : "rotate-180"}`}
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
            strokeWidth={2}
          >
            <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
          </svg>
        )}
      </div>

      {/* Card body */}
      {showBody && (
        <div className="px-4 py-3">
          {children}
        </div>
      )}
    </div>
  );
}

/* ─── Accordion ─── */

function Accordion({
  title,
  defaultOpen,
  className,
  children,
}: {
  title: string;
  defaultOpen: boolean;
  className?: string;
  children: React.ReactNode;
}) {
  const [open, setOpen] = useState(defaultOpen);

  return (
    <div className={`rounded-lg border border-gh-border overflow-hidden ${className || ""}`}>
      <button
        onClick={() => setOpen(!open)}
        className="w-full flex items-center justify-between px-4 py-2.5 bg-gh-surface hover:bg-gh-overlay transition text-sm font-medium text-gh-text-secondary cursor-pointer"
      >
        <span>{title}</span>
        <svg
          className={`w-4 h-4 transition-transform ${open ? "rotate-180" : ""}`}
          fill="none"
          viewBox="0 0 24 24"
          stroke="currentColor"
          strokeWidth={2}
        >
          <path strokeLinecap="round" strokeLinejoin="round" d="M19 9l-7 7-7-7" />
        </svg>
      </button>
      {open && (
        <div className="px-4 py-3 border-t border-gh-border">
          {children}
        </div>
      )}
    </div>
  );
}

/* ─── Sub-components ─── */

function AnsweredHearings({ hearings }: { hearings: TaskHearing[] }) {
  const answered = hearings.filter((h) => h.status === "answered");
  if (answered.length === 0) return null;

  return (
    <div className="space-y-2">
      {answered.map((h) => (
        <div key={h.id} className="text-sm space-y-1">
          <div className="text-[10px] text-gh-text-muted font-medium">
            {h.phase === "pre_plan" ? "事前ヒアリング" : "計画中ヒアリング"} (Round {h.round})
          </div>
          {h.questions.map((q) => {
            const answer = h.answers?.find((a) => a.index === q.index);
            return (
              <div key={q.index} className="flex gap-2">
                <span className="text-gh-text-muted shrink-0">Q{q.index}:</span>
                <span className="text-gh-text-secondary">{q.question}</span>
                <span className="text-gh-text ml-auto shrink-0">→ {answer?.answer || "未回答"}</span>
              </div>
            );
          })}
        </div>
      ))}
    </div>
  );
}

function LiveProgress({ messages, logs }: { messages: WsMessage[]; logs: ExecutionLog[] }) {
  const bottomRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages.length, logs.length]);

  // logs と ws メッセージを統合表示
  const hasLogs = logs.length > 0;

  return (
    <div className="max-h-48 overflow-auto space-y-0.5 text-sm font-mono">
      {hasLogs && logs.map((log) => (
        <div key={log.id} className="py-0.5">
          <span className="text-gh-text-muted">
            {new Date(log.created_at).toLocaleTimeString("ja-JP")}
          </span>{" "}
          <span className={
            log.level === "error" ? "text-gh-red"
              : log.level === "warn" ? "text-gh-orange"
              : "text-gh-green"
          }>
            [{log.phase}]
          </span>{" "}
          <span className="text-gh-text">{log.message}</span>
        </div>
      ))}
      {!hasLogs && messages.map((msg, i) => (
        <div key={i} className="py-0.5">
          <span className="font-mono text-gh-text-muted">[{msg.phase}]</span>{" "}
          <span className="text-gh-text">{msg.message}</span>
        </div>
      ))}
      <div ref={bottomRef} />
    </div>
  );
}

function CompletedCard({ task }: { task: Task }) {
  return (
    <div className="space-y-2">
      {task.pr_url && (
        <div className="flex items-center gap-2">
          <span className="text-sm text-gh-text-secondary">PR:</span>
          <a
            href={task.pr_url}
            target="_blank"
            rel="noopener noreferrer"
            className="text-sm text-gh-link hover:underline"
          >
            {task.pr_url}
          </a>
        </div>
      )}
      {!task.pr_url && task.proposal_type !== "development" && (
        <div className="text-sm text-gh-text-muted">
          {task.proposal_type === "investigation" && "調査完了 — 結果は計画内容セクションに保存"}
          {task.proposal_type === "operation" && "操作完了 — 結果は計画内容セクションに保存"}
          {task.proposal_type === "improvement" && "完了（変更なし）"}
        </div>
      )}
      {task.diff_stats && (
        <div className="text-xs text-gh-text-muted font-mono">{task.diff_stats}</div>
      )}
    </div>
  );
}

function ActionButton({
  onClick,
  variant,
  title,
  children,
}: {
  onClick: () => void;
  variant: "blue" | "green" | "outline-green" | "outline-red";
  title?: string;
  children: React.ReactNode;
}) {
  const classes = {
    blue: "bg-gh-blue/90 text-white hover:bg-gh-blue",
    green: "bg-gh-green/90 text-white hover:bg-gh-green",
    "outline-green": "border border-gh-green/40 text-gh-green hover:bg-gh-green/10",
    "outline-red": "border border-gh-red/40 text-gh-red hover:bg-gh-red/10",
  };

  return (
    <button
      onClick={onClick}
      title={title}
      className={`px-3 py-1.5 rounded-md text-sm font-medium transition ${classes[variant]}`}
    >
      {children}
    </button>
  );
}

function DoDChecklist({ dod }: { dod: string }) {
  const lines = dod.split("\n").filter((l) => l.trim().length > 0);
  return (
    <ul className="space-y-1">
      {lines.map((line, i) => {
        const trimmed = line.trim();
        const isChecked = trimmed.startsWith("- [x]") || trimmed.startsWith("- [X]");
        const isCheckbox = isChecked || trimmed.startsWith("- [ ]");
        const text = isCheckbox ? trimmed.slice(5).trim() : trimmed;
        return (
          <li key={i} className="flex items-start gap-2 text-sm">
            {isCheckbox ? (
              <span className={`mt-0.5 w-4 h-4 shrink-0 rounded border ${
                isChecked
                  ? "bg-gh-green/20 border-gh-green text-gh-green flex items-center justify-center"
                  : "border-gh-border"
              }`}>
                {isChecked && (
                  <svg className="w-3 h-3" fill="none" viewBox="0 0 24 24" stroke="currentColor" strokeWidth={3}>
                    <path strokeLinecap="round" strokeLinejoin="round" d="M5 13l4 4L19 7" />
                  </svg>
                )}
              </span>
            ) : (
              <span className="mt-0.5 w-4 shrink-0 text-gh-text-muted">•</span>
            )}
            <span className={isChecked ? "text-gh-text-muted line-through" : "text-gh-text-secondary"}>{text}</span>
          </li>
        );
      })}
    </ul>
  );
}

function Spinner() {
  return (
    <svg className="animate-spin h-4 w-4 text-gh-orange" viewBox="0 0 24 24" fill="none">
      <circle className="opacity-25" cx="12" cy="12" r="10" stroke="currentColor" strokeWidth="4" />
      <path className="opacity-75" fill="currentColor" d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4z" />
    </svg>
  );
}
