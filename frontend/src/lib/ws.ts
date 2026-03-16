import type { WsMessage, ScanWsMessage } from "@/types";

const WS_BASE = process.env.NEXT_PUBLIC_WS_URL || "ws://localhost:8100";

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
