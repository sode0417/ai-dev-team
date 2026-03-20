"use client";

import { useEffect, useState, useCallback } from "react";
import { connectNotificationWs } from "@/lib/ws";
import type { MergeEvent } from "@/types";

interface Notification {
  id: number;
  event: MergeEvent;
  timestamp: number;
}

let notifId = 0;

export function MergeNotificationProvider() {
  const [notifications, setNotifications] = useState<Notification[]>([]);

  const addNotification = useCallback((event: MergeEvent) => {
    const id = ++notifId;
    setNotifications((prev) => [...prev, { id, event, timestamp: Date.now() }]);

    // failed 以外は 8 秒で自動消去
    if (event.event !== "failed") {
      setTimeout(() => {
        setNotifications((prev) => prev.filter((n) => n.id !== id));
      }, 8000);
    }
  }, []);

  const dismiss = useCallback((id: number) => {
    setNotifications((prev) => prev.filter((n) => n.id !== id));
  }, []);

  useEffect(() => {
    let ws: WebSocket | null = null;
    let reconnectTimer: ReturnType<typeof setTimeout>;

    function connect() {
      ws = connectNotificationWs(
        (msg) => addNotification(msg),
        () => {
          // 自動再接続（5秒後）
          reconnectTimer = setTimeout(connect, 5000);
        }
      );
    }

    connect();

    return () => {
      clearTimeout(reconnectTimer);
      ws?.close();
    };
  }, [addNotification]);

  if (notifications.length === 0) return null;

  return (
    <div className="fixed bottom-4 right-4 z-50 flex flex-col gap-2 max-w-sm">
      {notifications.map((n) => (
        <NotificationToast key={n.id} notification={n} onDismiss={dismiss} />
      ))}
    </div>
  );
}

const EVENT_STYLES: Record<
  string,
  { bg: string; border: string; icon: string; label: string }
> = {
  merged: {
    bg: "bg-gh-green/10",
    border: "border-gh-green/30",
    icon: "text-gh-green",
    label: "マージ完了",
  },
  conflict_resolved: {
    bg: "bg-gh-orange/10",
    border: "border-gh-orange/30",
    icon: "text-gh-orange",
    label: "コンフリクト解消",
  },
  conflict: {
    bg: "bg-gh-orange/10",
    border: "border-gh-orange/30",
    icon: "text-gh-orange",
    label: "コンフリクト検出",
  },
  failed: {
    bg: "bg-gh-red/10",
    border: "border-gh-red/30",
    icon: "text-gh-red",
    label: "マージ失敗",
  },
  auto_merge_enabled: {
    bg: "bg-gh-blue/10",
    border: "border-gh-blue/30",
    icon: "text-gh-blue",
    label: "Auto-merge有効",
  },
};

function NotificationToast({
  notification,
  onDismiss,
}: {
  notification: Notification;
  onDismiss: (id: number) => void;
}) {
  const { event } = notification;
  const style = EVENT_STYLES[event.event] || EVENT_STYLES.failed;

  return (
    <div
      className={`${style.bg} ${style.border} border rounded-lg p-3 shadow-lg animate-in slide-in-from-right`}
    >
      <div className="flex items-start gap-2">
        <span className={`${style.icon} text-sm shrink-0 mt-0.5`}>
          {event.event === "merged" && "\u2713"}
          {event.event === "conflict_resolved" && "\u2713"}
          {event.event === "conflict" && "\u26A0"}
          {event.event === "failed" && "\u2717"}
          {event.event === "auto_merge_enabled" && "\u2713"}
        </span>
        <div className="flex-1 min-w-0">
          <div className="flex items-center justify-between gap-2">
            <span className={`text-xs font-semibold ${style.icon}`}>
              {style.label}
            </span>
            <button
              onClick={() => onDismiss(notification.id)}
              className="text-gh-text-muted hover:text-gh-text text-xs shrink-0"
            >
              &times;
            </button>
          </div>
          <p className="text-sm text-gh-text font-medium truncate mt-0.5">
            {event.task_title}
          </p>
          <p className="text-xs text-gh-text-muted mt-0.5 line-clamp-2">
            {event.message}
          </p>
        </div>
      </div>
    </div>
  );
}
