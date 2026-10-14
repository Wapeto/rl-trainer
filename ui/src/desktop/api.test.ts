import { describe, expect, it, vi } from "vitest";
import { DesktopError, createDesktopApi, httpTransport, tauriTransport, type TauriGlobal } from "./api";

function okFetch(data: unknown) {
  return vi.fn().mockResolvedValue(new Response(JSON.stringify({ ok: true, data })));
}

describe("httpTransport", () => {
  it("posts the arguments to the command with the desktop header", async () => {
    const fetchImpl = okFetch([]);
    const api = createDesktopApi(httpTransport(fetchImpl));
    await api.sessions("wall", 50, 50);
    const [url, init] = fetchImpl.mock.calls[0];
    expect(url).toBe("/desktop-api/sessions");
    expect(init.method).toBe("POST");
    expect(init.headers["X-RL-Desktop"]).toBe("1");
    expect(JSON.parse(init.body)).toEqual({ search: "wall", offset: 50, limit: 50 });
  });

  it("returns the app's error message", async () => {
    const fetchImpl = vi.fn().mockResolvedValue(
      new Response(JSON.stringify({ ok: false, error: { message: "there is no earlier session" } }), { status: 400 }),
    );
    const err = await createDesktopApi(httpTransport(fetchImpl)).mergeSessions(1, "previous").catch((e) => e);
    expect(err).toBeInstanceOf(DesktopError);
    expect(err.message).toBe("there is no earlier session");
  });

  it("says plainly when the app is not running, and has no autostart", async () => {
    const api = createDesktopApi(httpTransport(vi.fn().mockRejectedValue(new TypeError("fetch failed"))));
    expect((await api.about().catch((e) => e)).message).toBe("The app is not running.");
    expect(await api.autostart()).toBeNull();
  });
});

describe("tauriTransport", () => {
  function tauri(invoke: ReturnType<typeof vi.fn>): TauriGlobal {
    return { core: { invoke }, event: { listen: vi.fn() } };
  }

  it("calls the one desktop command with the command name and arguments", async () => {
    const invoke = vi.fn().mockResolvedValue(42);
    const pack = { code: "1111", name: "x", target_rate: 0.65, shot_count: null, tags: [], notes: "", goalie: false };
    expect(await createDesktopApi(tauriTransport(tauri(invoke))).addPack(pack)).toBe(42);
    expect(invoke).toHaveBeenCalledWith("desktop", { command: "add_pack", args: { pack } });
  });

  it("sends the progress commands with their arguments", async () => {
    const call = vi.fn().mockResolvedValue({});
    const api = createDesktopApi({ call, autostart: null });
    await api.progressList("days90");
    await api.progressOverview("all");
    await api.progressMode(2, "days30");
    await api.progressPack(7, "days90");
    await api.modeBenchmark(3);
    expect(call.mock.calls).toEqual([
      ["progress_list", { range: "days90" }],
      ["progress_overview", { range: "all" }],
      ["progress_mode", { team_size: 2, range: "days30" }],
      ["progress_pack", { pack_id: 7, range: "days90" }],
      ["mode_benchmark", { team_size: 3 }],
    ]);
  });

  it("sends the coach commands", async () => {
    const call = vi.fn().mockResolvedValue(null);
    const api = createDesktopApi({ call, autostart: null });
    await api.sessionReview(12);
    await api.replaysToSave();
    await api.replayStatus();
    await api.sessions("", 0, 50, true);
    await api.sessions("x", 0, 50);
    expect(call.mock.calls).toEqual([
      ["session_review", { id: 12 }],
      ["replays_to_save", {}],
      ["replay_status", {}],
      ["sessions", { search: "", offset: 0, limit: 50, to_save: true }],
      ["sessions", { search: "x", offset: 0, limit: 50 }],
    ]);
  });

  it("turns a rejected call into a DesktopError", async () => {
    const invoke = vi.fn().mockRejectedValue("this session is still being recorded");
    const err = await createDesktopApi(tauriTransport(tauri(invoke))).deleteSession(3).catch((e) => e);
    expect(err).toBeInstanceOf(DesktopError);
    expect(err.message).toBe("this session is still being recorded");
  });

  it("reads and sets autostart through its own commands", async () => {
    const invoke = vi.fn().mockResolvedValue(true);
    const api = createDesktopApi(tauriTransport(tauri(invoke)));
    expect(await api.autostart()).toBe(true);
    await api.setAutostart(false);
    expect(invoke).toHaveBeenCalledWith("get_autostart");
    expect(invoke).toHaveBeenCalledWith("set_autostart", { enabled: false });
  });

  it("quits and restarts the app through their own commands", async () => {
    const invoke = vi.fn().mockResolvedValue(null);
    const api = createDesktopApi(tauriTransport(tauri(invoke)));
    expect(api.canControlApp).toBe(true);
    await api.quitApp();
    await api.restartApp();
    expect(invoke).toHaveBeenCalledWith("quit_app");
    expect(invoke).toHaveBeenCalledWith("restart_app");
  });

  it("cannot quit or restart the app from a browser", () => {
    expect(createDesktopApi(httpTransport(vi.fn())).canControlApp).toBe(false);
  });
});
