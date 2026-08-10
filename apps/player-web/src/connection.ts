import type { Credentials, ServerEvent } from "./types";

export const parseServerEvent = (text: string): ServerEvent | null => {
  try {
    const event = JSON.parse(text) as Partial<ServerEvent>;
    if (event.type === "authenticated" || event.type === "pong") return event as ServerEvent;
    if (event.type === "room_snapshot" && "snapshot" in event) return event as ServerEvent;
    if (event.type === "error" && "message" in event && "code" in event) return event as ServerEvent;
    return null;
  } catch {
    return null;
  }
};

export class PlayerConnection {
  private socket: WebSocket | null = null;
  private retry = 0;
  private stopped = false;
  private reconnectTimer: number | null = null;
  private pingTimer: number | null = null;

  constructor(
    private readonly url: string,
    private readonly credentials: Credentials,
    private readonly onEvent: (event: ServerEvent) => void,
    private readonly onStatus: (connected: boolean) => void,
  ) {}

  connect() {
    this.stopped = false;
    this.open();
  }

  send(event: object) {
    if (this.socket?.readyState === WebSocket.OPEN) {
      this.socket.send(JSON.stringify(event));
      return true;
    }
    return false;
  }

  stop() {
    this.stopped = true;
    if (this.reconnectTimer !== null) window.clearTimeout(this.reconnectTimer);
    if (this.pingTimer !== null) window.clearInterval(this.pingTimer);
    this.socket?.close();
    this.socket = null;
  }

  private open() {
    if (this.stopped) return;
    const socket = new WebSocket(this.url);
    this.socket = socket;
    socket.onopen = () => {
      this.retry = 0;
      socket.send(
        JSON.stringify({
          type: "authenticate_player",
          roomCode: this.credentials.roomCode,
          playerId: this.credentials.playerId,
          sessionToken: this.credentials.sessionToken,
        }),
      );
      this.pingTimer = window.setInterval(() => this.send({ type: "ping" }), 20_000);
    };
    socket.onmessage = (message) => {
      const event = parseServerEvent(String(message.data));
      if (event) {
        if (event.type === "authenticated") this.onStatus(true);
        this.onEvent(event);
      }
    };
    socket.onerror = () => socket.close();
    socket.onclose = () => {
      if (this.pingTimer !== null) window.clearInterval(this.pingTimer);
      this.onStatus(false);
      if (!this.stopped) {
        const delays = [1_000, 2_000, 4_000, 8_000, 10_000];
        const delay = delays[Math.min(this.retry, delays.length - 1)];
        this.retry += 1;
        this.reconnectTimer = window.setTimeout(() => this.open(), delay);
      }
    };
  }
}

