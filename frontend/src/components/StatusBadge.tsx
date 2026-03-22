import type { TaskStatus } from "@/types";

const statusConfig: Record<TaskStatus, { label: string; className: string }> = {
  proposed: { label: "Proposed", className: "bg-gh-text-muted/20 text-gh-text-secondary" },
  approved: { label: "Approved", className: "bg-gh-blue/15 text-gh-blue" },
  queued: { label: "Queued", className: "bg-gh-purple/15 text-gh-purple" },
  hearing: { label: "ヒアリング中", className: "bg-gh-orange/15 text-gh-orange" },
  planning: { label: "Planning", className: "bg-gh-orange/15 text-gh-orange" },
  awaiting_approval: { label: "承認待ち", className: "bg-gh-orange/20 text-gh-orange" },
  executing: { label: "Executing", className: "bg-gh-orange/20 text-gh-orange" },
  reviewing: { label: "Reviewing", className: "bg-gh-purple/15 text-gh-purple" },
  pending_completion: { label: "完了確認待ち", className: "bg-gh-green/10 text-gh-green/80 ring-1 ring-gh-green/30" },
  completed: { label: "Completed", className: "bg-gh-green/15 text-gh-green" },
  failed: { label: "Failed", className: "bg-gh-red/15 text-gh-red" },
  cancelled: { label: "Cancelled", className: "bg-gh-text-muted/15 text-gh-text-muted" },
  blocked: { label: "Blocked", className: "bg-gh-red/10 text-gh-red/70" },
};

export function StatusBadge({ status }: { status: TaskStatus }) {
  const config = statusConfig[status] || {
    label: status,
    className: "bg-gh-text-muted/20 text-gh-text-secondary",
  };

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded-full text-xs font-medium ${config.className}`}
    >
      {config.label}
    </span>
  );
}
