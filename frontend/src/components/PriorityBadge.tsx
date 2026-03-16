import type { TaskPriority } from "@/types";

const priorityConfig: Record<TaskPriority, { label: string; className: string }> = {
  critical: { label: "Critical", className: "bg-gh-red/15 text-gh-red" },
  high: { label: "High", className: "bg-gh-orange/15 text-gh-orange" },
  medium: { label: "Medium", className: "bg-gh-blue/10 text-gh-text-secondary" },
  low: { label: "Low", className: "bg-gh-text-muted/15 text-gh-text-muted" },
};

export function PriorityBadge({ priority }: { priority: TaskPriority }) {
  const config = priorityConfig[priority] || {
    label: priority,
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
