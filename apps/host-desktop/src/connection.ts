import type { HostCredentials, ServerEvent } from "./types";

function parseEvent(text: string): ServerEvent | null {
  try {
    const event = JSON.parse(text) as ServerEvent;
    return ["authenticated", "room_snapshot", "error", "pong"].includes(event.type) ? event : null;
  } catch { return null; }
}

export class HostConnection {
  private socket: WebSocket | null = null;
  private retry = 0;
  private stopped = false;
  private reconnectTimer?: number;
  private pingTimer?: number;

  constructor(private url: string, private credentials: HostCredentials, private onEvent: (event: ServerEvent) => void, private onStatus: (online: boolean) => void) {}

  connect() { this.stopped = false; this.open(); }
  stop() { this.stopped = true; window.clearTimeout(this.reconnectTimer); window.clearInterval(this.pingTimer); this.socket?.close(); }
  send(event: object) {
    if (this.socket?.readyState !== WebSocket.OPEN) return false;
    this.socket.send(JSON.stringify(event));
    return true;
  }
  private open() {
    if (this.stopped) return;
    const socket = new WebSocket(this.url);
    this.socket = socket;
    socket.onopen = () => {
      this.retry = 0;
      socket.send(JSON.stringify({ type: "authenticate_host", roomCode: this.credentials.roomCode, hostToken: this.credentials.hostToken }));
      this.pingTimer = window.setInterval(() => this.send({ type: "ping" }), 20_000);
    };
    socket.onmessage = (message) => {
      const event = parseEvent(String(message.data));
      if (!event) return;
      if (event.type === "authenticated") this.onStatus(true);
      this.onEvent(event);
    };
    socket.onerror = () => socket.close();
    socket.onclose = () => {
      window.clearInterval(this.pingTimer);
      this.onStatus(false);
      if (!this.stopped) {
        const delays = [1_000, 2_000, 4_000, 8_000, 10_000];
        this.reconnectTimer = window.setTimeout(() => this.open(), delays[Math.min(this.retry++, 4)]);
      }
    };
  }
}

