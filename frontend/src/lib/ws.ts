import type { WsMessage, ScanWsMessage, SprintWsMessage } from "@/types";

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

export function connectTaskWs(
  taskId: string,
  onMessage: (msg: WsMessage) => void,
  onClose?: () => void
): WebSocket {
  const ws = new WebSocket(`${WS_BASE}/ws/executions/${taskId}`);

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
  const ws = new WebSocket(`${WS_BASE}/ws/scans/${scanId}`);

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
  const ws = new WebSocket(`${WS_BASE}/ws/sprints/${sprintId}`);

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
