import type { AppSnapshot } from "@shared/generated/types";
import type { DesktopApi, TauriGlobal } from "./api";

export interface SnapshotFeed {
  getState(): AppSnapshot | null;
  subscribe(listener: () => void): () => void;
}

/** Holds the latest app snapshot. An older one (a late reply) never replaces a newer one. */
export class LiveFeed implements SnapshotFeed {
  private state: AppSnapshot | null = null;
  private readonly listeners = new Set<() => void>();

  getState = (): AppSnapshot | null => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  set(next: AppSnapshot): void {
    if (this.state && next.version < this.state.version) return;
    this.state = next;
    this.listeners.forEach((listener) => listener());
  }
}

/** In the app: every `live` event the shell emits, plus the current snapshot to start with. */
export async function followTauri(feed: LiveFeed, tauri: TauriGlobal, api: DesktopApi): Promise<void> {
  await tauri.event.listen("live", (e) => feed.set(e.payload as AppSnapshot));
  feed.set(await api.snapshot());
}

/** In a browser: ask every `everyMs`. A failed ask keeps the last snapshot; the next one retries.
 * Returns a function that stops polling. */
export function poll(feed: LiveFeed, api: DesktopApi, everyMs = 1000): () => void {
  const tick = () => {
    api.snapshot().then(
      (snapshot) => feed.set(snapshot),
      () => undefined,
    );
  };
  tick();
  const id = setInterval(tick, everyMs);
  return () => clearInterval(id);
}
