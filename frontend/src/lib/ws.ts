import type { WsMessage, ScanWsMessage, SprintWsMessage, MergeEvent } from "@/types";
import { getAccessToken } from "./auth";

function getWsBase(): string {
  if (process.env.NEXT_PUBLIC_WS_URL) return process.env.NEXT_PUBLIC_WS_URL;
  if (typeof window !== "undefined" && window.location.hostname !== "localhost") {
    const protocol = window.location.protocol === "https:" ? "wss:" : "ws:";
    // devteam.sode-ai.com → devteam-api.sode-ai.com
    const apiHost = window.location.hostname.replace(
      /^([^.]+)\./,
      "$1-api."
    );
    return `${protocol}//${apiHost}`;
  }
  return "ws://localhost:8100";
}

const WS_BASE = getWsBase();

function buildWsUrl(path: string): string {
  const token = getAccessToken();
  const url = `${WS_BASE}${path}`;
  return token ? `${url}?token=${encodeURIComponent(token)}` : url;
}

export function connectTaskWs(
  taskId: string,
  onMessage: (msg: WsMessage) => void,
  onClose?: () => void
): WebSocket {
  const ws = new WebSocket(buildWsUrl(`/ws/executions/${taskId}`));

  ws.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data) as WsMessage;
      onMessage(data);
    } catch {
      console.warn("Invalid WS message:", event.data);
    }
  };

  ws.onclose = () => {
    onClose?.();
  };

  return ws;
}

export function connectScanWs(
  scanId: string,
  onMessage: (msg: ScanWsMessage) => void,
  onClose?: () => void
): WebSocket {
  const ws = new WebSocket(buildWsUrl(`/ws/scans/${scanId}`));

  ws.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data) as ScanWsMessage;
      onMessage(data);
    } catch {
      console.warn("Invalid WS message:", event.data);
    }
  };

  ws.onclose = () => {
    onClose?.();
  };

  return ws;
}

export function connectSprintWs(
  sprintId: string,
  onMessage: (msg: SprintWsMessage) => void,
  onClose?: () => void
): WebSocket {
  const ws = new WebSocket(buildWsUrl(`/ws/sprints/${sprintId}`));

  ws.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data) as SprintWsMessage;
      onMessage(data);
    } catch {
      console.warn("Invalid WS message:", event.data);
    }
  };

  ws.onclose = () => {
    onClose?.();
  };

  return ws;
}

export function connectNotificationWs(
  onMessage: (msg: MergeEvent) => void,
  onClose?: () => void
): WebSocket {
  const ws = new WebSocket(buildWsUrl("/ws/notifications"));

  ws.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data) as MergeEvent;
      if (data.type === "merge_event") {
        onMessage(data);
      }
    } catch {
      console.warn("Invalid notification WS message:", event.data);
    }
  };

  ws.onclose = () => {
    onClose?.();
  };

  return ws;
}
