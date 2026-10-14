import blankMp4 from "./media/blank.mp4";
import blankWebm from "./media/blank.webm";

export type AwakeState = "off" | "on";

export interface WakeLockLike {
  /** Resolves to the lock; its `release` event fires when the browser drops it. */
  request(type: "screen"): Promise<{ addEventListener(type: "release", fn: () => void): void }>;
}

export interface VideoLike {
  muted: boolean;
  loop: boolean;
  playsInline: boolean;
  play(): Promise<void>;
  addEventListener(type: "pause", fn: () => void): void;
}

export interface KeepAwakeDeps {
  /** The Screen Wake Lock API; `null` where the page may not use it (plain HTTP). */
  wakeLock: WakeLockLike | null;
  createVideo(): VideoLike;
  doc: { visibilityState: DocumentVisibilityState; addEventListener(type: "visibilitychange", fn: () => void): void };
}

/** Keeps the phone screen on while the tracker page is open. Browsers only allow it after a
 * tap, and only allow the Wake Lock API on secure pages; on the LAN's plain HTTP a muted,
 * looping, invisible video does the same job. */
export class KeepAwake {
  private state: AwakeState = "off";
  private readonly listeners = new Set<() => void>();
  private video: VideoLike | null = null;
  private useVideo = false;
  private starting: Promise<void> | null = null;

  constructor(private readonly deps: KeepAwakeDeps) {
    // Hiding the page drops the lock and pauses the video: take them again on return.
    deps.doc.addEventListener("visibilitychange", () => this.lost());
  }

  getState = (): AwakeState => this.state;

  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => {
      this.listeners.delete(listener);
    };
  };

  /** Call from a user gesture (tap). */
  enable(): Promise<void> {
    if (this.state === "on") return Promise.resolve();
    // Two quick taps start it once.
    this.starting ??= this.acquire()
      .then((ok) => this.set(ok ? "on" : "off"))
      .finally(() => {
        this.starting = null;
      });
    return this.starting;
  }

  /** The lock was dropped or the video paused (page hidden, battery saver, interruption): take
   * it again while the page is shown, and show the hint again if that is refused. */
  private lost(): void {
    if (this.state !== "on" || this.deps.doc.visibilityState !== "visible") return;
    void this.acquire().then((ok) => {
      if (!ok) this.set("off");
    });
  }

  private set(next: AwakeState): void {
    if (next === this.state) return;
    this.state = next;
    this.listeners.forEach((listener) => listener());
  }

  private async acquire(): Promise<boolean> {
    if (this.deps.wakeLock && !this.useVideo) {
      try {
        const lock = await this.deps.wakeLock.request("screen");
        lock.addEventListener("release", () => this.lost());
        return true;
      } catch {
        this.useVideo = true; // refused (e.g. battery saver): fall back to the video
      }
    }
    return this.playVideo();
  }

  private async playVideo(): Promise<boolean> {
    if (!this.video) {
      const video = this.deps.createVideo();
      video.muted = true;
      video.loop = true;
      video.playsInline = true;
      video.addEventListener("pause", () => this.lost());
      this.video = video;
    }
    try {
      await this.video.play();
      return true;
    } catch {
      return false;
    }
  }
}

/** The blank clip this browser can play: H.264 MP4 where supported (iOS needs it), else VP8
 * WebM (open-source Chromium builds have no H.264). */
export function videoSource(canPlayType: (type: string) => string): string {
  return canPlayType('video/mp4; codecs="avc1.42E01E"') ? blankMp4 : blankWebm;
}

/** The real browser pieces: the Wake Lock API on secure pages, else a hidden video element. */
export function browserKeepAwakeDeps(): KeepAwakeDeps {
  const nav = navigator as Navigator & { wakeLock?: WakeLockLike };
  return {
    wakeLock: window.isSecureContext && nav.wakeLock ? nav.wakeLock : null,
    createVideo: () => {
      const video = document.createElement("video");
      video.src = videoSource((type) => video.canPlayType(type));
      video.setAttribute("aria-hidden", "true");
      video.className = "keep-awake-video";
      // Chrome only keeps the screen on for a muted video that fills over 20% of the viewport
      // and is 75% in view (blink VideoWakeLock checks geometry, not opacity or stacking): so
      // it covers the screen, invisible, behind everything and never in the way of a tap.
      Object.assign(video.style, {
        position: "fixed",
        top: "0",
        left: "0",
        width: "100vw",
        height: "100vh",
        opacity: "0",
        pointerEvents: "none",
        zIndex: "-1",
      });
      document.body.appendChild(video);
      return video;
    },
    doc: document,
  };
}
