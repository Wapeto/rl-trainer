import type { AppSnapshot } from "@shared/generated/types";

export type ConnState = "connecting" | "live" | "reconnecting" | "unpaired";

export interface LiveState {
  conn: ConnState;
  snapshot: AppSnapshot | null;
}

export interface LiveSource {
  getState(): LiveState;
  subscribe(listener: () => void): () => void;
}

export const RETRY_DELAYS_MS = [1000, 2000, 5000];
export const UNAUTHORIZED = 4401;

export interface SocketLike {
  send(data: string): void;
  close(): void;
  onopen: ((ev: Event) => void) | null;
  onmessage: ((ev: MessageEvent) => void) | null;
  onclose: ((ev: CloseEvent) => void) | null;
}

export interface LiveStoreOptions {
  token: string | null;
  url?: string;
  connect?: (url: string) => SocketLike;
  schedule?: (fn: () => void, ms: number) => void;
}

function defaultUrl(): string {
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  return `${proto}://${window.location.host}/api/live/ws`;
}

function parse(data: unknown): { type?: string; data?: AppSnapshot } | null {
  if (typeof data !== "string") return null;
  try {
    return JSON.parse(data);
  } catch {
    return null;
  }
}

/** Owns the live WebSocket: auth, snapshots, connection state and reconnect backoff. */
export class LiveStore implements LiveSource {
  private state: LiveState;
  private readonly listeners = new Set<() => void>();
  private socket: SocketLike | null = null;
  private attempt = 0;
  private stopped = false;
  private readonly opts: LiveStoreOptions;

  constructor(opts: LiveStoreOptions) {
    this.opts = opts;
    this.state = { conn: opts.token ? "connecting" : "unpaired", snapshot: null };
  }

  getState = (): LiveState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  start(): void {
    if (this.opts.token && !this.socket && !this.stopped) this.open();
  }

  /** Drops the current socket and reconnects at once. After the phone sleeps or changes network
   * the old socket can be half-open and never report a close, so the page calls this when it
   * becomes visible or comes back online. */
  resume(): void {
    if (!this.opts.token || this.stopped || this.state.conn === "unpaired") return;
    const old = this.socket;
    if (old) {
      old.onopen = old.onmessage = old.onclose = null;
      old.close();
      this.socket = null;
    }
    this.attempt = 0;
    this.open();
  }

  stop(): void {
    this.stopped = true;
    this.socket?.close();
    this.socket = null;
  }

  /** The PC refused the token on an action: show the pairing screen and stop reconnecting. */
  markUnpaired(): void {
    this.stopped = true;
    const old = this.socket;
    if (old) {
      old.onopen = old.onmessage = old.onclose = null;
      old.close();
      this.socket = null;
    }
    this.set({ conn: "unpaired" });
  }

  private set(next: Partial<LiveState>): void {
    this.state = { ...this.state, ...next };
    this.listeners.forEach((listener) => listener());
  }

  private open(): void {
    const connect = this.opts.connect ?? ((url: string) => new WebSocket(url));
    const socket = connect(this.opts.url ?? defaultUrl());
    this.socket = socket;
    socket.onopen = () => socket.send(JSON.stringify({ type: "auth", token: this.opts.token }));
    socket.onmessage = (ev) => {
      const msg = parse(ev.data);
      if (msg?.type === "snapshot" && msg.data) {
        this.attempt = 0;
        this.set({ conn: "live", snapshot: msg.data });
      }
    };
    socket.onclose = (ev) => {
      this.socket = null;
      if (this.stopped) return;
      if (ev.code === UNAUTHORIZED) {
        this.set({ conn: "unpaired" });
        return;
      }
      this.set({ conn: this.state.snapshot ? "reconnecting" : "connecting" });
      const delay = RETRY_DELAYS_MS[Math.min(this.attempt, RETRY_DELAYS_MS.length - 1)];
      this.attempt += 1;
      const schedule = this.opts.schedule ?? ((fn: () => void, ms: number) => void setTimeout(fn, ms));
      schedule(() => {
        if (!this.stopped && !this.socket) this.open();
      }, delay);
    };
  }
}
