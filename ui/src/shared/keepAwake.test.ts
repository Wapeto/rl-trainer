import { describe, expect, it, vi } from "vitest";
import { browserKeepAwakeDeps, KeepAwake, videoSource, type KeepAwakeDeps, type VideoLike, type WakeLockLike } from "./keepAwake";

function fakeDoc() {
  let onChange: (() => void) | null = null;
  const doc = {
    visibilityState: "visible" as DocumentVisibilityState,
    addEventListener: (_type: "visibilitychange", fn: () => void) => {
      onChange = fn;
    },
  };
  return {
    doc,
    hide() {
      doc.visibilityState = "hidden";
      onChange?.();
    },
    show() {
      doc.visibilityState = "visible";
      onChange?.();
    },
  };
}

function fakeLock() {
  let onRelease: (() => void) | null = null;
  const lock: WakeLockLike = {
    request: vi.fn().mockImplementation(async () => ({
      addEventListener: (_type: "release", fn: () => void) => {
        onRelease = fn;
      },
    })),
  };
  return { lock, release: () => onRelease?.() };
}

function fakeVideo(play = vi.fn().mockResolvedValue(undefined)): VideoLike & { pauseNow(): void } {
  let onPause: (() => void) | null = null;
  return {
    muted: false,
    loop: false,
    playsInline: false,
    play,
    addEventListener: (_type: "pause", fn: () => void) => {
      onPause = fn;
    },
    pauseNow: () => onPause?.(),
  };
}

function setup(over: Partial<KeepAwakeDeps> = {}) {
  const { doc, hide, show } = fakeDoc();
  const video = fakeVideo();
  const deps: KeepAwakeDeps = { wakeLock: null, createVideo: vi.fn(() => video), doc, ...over };
  return { awake: new KeepAwake(deps), deps, video, hide, show };
}

describe("browserKeepAwakeDeps", () => {
  it("makes a muted video that covers the whole screen, invisible and out of the way", () => {
    // Chrome keeps the screen on for a muted video only when it fills over 20% of the viewport
    // and is 75% in view; opacity and stacking are not checked (blink VideoWakeLock).
    const video = browserKeepAwakeDeps().createVideo() as unknown as HTMLVideoElement;
    expect(video.style.position).toBe("fixed");
    expect([video.style.top, video.style.left]).toEqual(["0px", "0px"]);
    expect([video.style.width, video.style.height]).toEqual(["100vw", "100vh"]);
    expect(video.style.opacity).toBe("0");
    expect(video.style.pointerEvents).toBe("none");
    expect(video.style.zIndex).toBe("-1");
    video.remove();
  });
});

describe("videoSource", () => {
  it("prefers the MP4 where H.264 plays and falls back to WebM", () => {
    expect(videoSource((type) => (type.startsWith("video/mp4") ? "probably" : ""))).toMatch(/\.mp4$/);
    expect(videoSource((type) => (type.startsWith("video/webm") ? "maybe" : ""))).toMatch(/\.webm$/);
  });
});

describe("KeepAwake", () => {
  it("uses the screen wake lock when the page is allowed to", async () => {
    const { lock } = fakeLock();
    const { awake, deps } = setup({ wakeLock: lock });
    await awake.enable();
    expect(lock.request).toHaveBeenCalledWith("screen");
    expect(deps.createVideo).not.toHaveBeenCalled();
    expect(awake.getState()).toBe("on");
  });

  it("falls back to a muted, looping, inline video on plain HTTP", async () => {
    const { awake, video } = setup();
    await awake.enable();
    expect(video).toMatchObject({ muted: true, loop: true, playsInline: true });
    expect(video.play).toHaveBeenCalledTimes(1);
    expect(awake.getState()).toBe("on");
  });

  it("falls back to the video when the lock is refused", async () => {
    const lock: WakeLockLike = { request: vi.fn().mockRejectedValue(new Error("NotAllowedError")) };
    const { awake, video } = setup({ wakeLock: lock });
    await awake.enable();
    expect(video.play).toHaveBeenCalled();
    expect(awake.getState()).toBe("on");
  });

  it("takes the lock again when the page comes back", async () => {
    const { lock, release } = fakeLock();
    const { awake, hide, show } = setup({ wakeLock: lock });
    await awake.enable();
    hide();
    release(); // the browser drops the lock while the page is hidden
    show();
    await vi.waitFor(() => expect(lock.request).toHaveBeenCalledTimes(2));
  });

  it("restarts the video when the page comes back", async () => {
    const { awake, video, show } = setup();
    await awake.enable();
    show();
    await vi.waitFor(() => expect(video.play).toHaveBeenCalledTimes(2));
  });

  it("stays off when playback is refused, and can be tried again", async () => {
    const play = vi.fn().mockRejectedValueOnce(new Error("NotAllowedError")).mockResolvedValue(undefined);
    const video = fakeVideo(play);
    const { awake } = setup({ createVideo: () => video });
    await awake.enable();
    expect(awake.getState()).toBe("off");
    await awake.enable();
    expect(awake.getState()).toBe("on");
  });

  it("shows the hint again when the screen can no longer be kept on after a return", async () => {
    const play = vi.fn().mockResolvedValueOnce(undefined).mockRejectedValue(new Error("NotAllowedError"));
    const video = fakeVideo(play);
    const { awake, show } = setup({ createVideo: () => video });
    await awake.enable();
    show();
    await vi.waitFor(() => expect(awake.getState()).toBe("off"));
  });

  it("restarts the video when the system pauses it while the page is shown", async () => {
    const { awake, video } = setup();
    await awake.enable();
    (video as ReturnType<typeof fakeVideo>).pauseNow();
    await vi.waitFor(() => expect(video.play).toHaveBeenCalledTimes(2));
  });

  it("takes the lock again when it is dropped while the page is shown", async () => {
    const { lock, release } = fakeLock();
    const { awake } = setup({ wakeLock: lock });
    await awake.enable();
    release(); // e.g. battery saver turned on
    await vi.waitFor(() => expect(lock.request).toHaveBeenCalledTimes(2));
    expect(awake.getState()).toBe("on");
  });

  it("starts only once for two quick taps", async () => {
    const { lock } = fakeLock();
    const { awake } = setup({ wakeLock: lock });
    await Promise.all([awake.enable(), awake.enable()]);
    expect(lock.request).toHaveBeenCalledTimes(1);
  });

  it("notifies subscribers and does nothing more once on", async () => {
    const { awake, video } = setup();
    const listener = vi.fn();
    awake.subscribe(listener);
    await awake.enable();
    await awake.enable();
    expect(video.play).toHaveBeenCalledTimes(1);
    expect(listener).toHaveBeenCalledTimes(1);
  });
});
