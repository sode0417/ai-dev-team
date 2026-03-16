import type { TaskStatus } from "@/types";

const statusConfig: Record<TaskStatus, { label: string; className: string }> = {
  proposed: { label: "Proposed", className: "bg-slate-100 text-slate-700" },
  approved: { label: "Approved", className: "bg-blue-100 text-blue-700" },
  queued: { label: "Queued", className: "bg-indigo-100 text-indigo-700" },
  planning: { label: "Planning", className: "bg-yellow-100 text-yellow-700" },
  executing: { label: "Executing", className: "bg-orange-100 text-orange-700" },
  reviewing: { label: "Reviewing", className: "bg-purple-100 text-purple-700" },
  completed: { label: "Completed", className: "bg-green-100 text-green-700" },
  failed: { label: "Failed", className: "bg-red-100 text-red-700" },
  cancelled: { label: "Cancelled", className: "bg-gray-100 text-gray-500" },
  blocked: { label: "Blocked", className: "bg-red-50 text-red-500" },
};

export function StatusBadge({ status }: { status: TaskStatus }) {
  const config = statusConfig[status] || {
    label: status,
    className: "bg-slate-100 text-slate-700",
  };

  return (
    <span
      className={`inline-flex items-center px-2 py-0.5 rounded text-xs font-medium ${config.className}`}
    >
      {config.label}
    </span>
  );
}
