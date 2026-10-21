import type {
  About,
  AppSnapshot,
  BackupInfo,
  Benchmark,
  MatchReplay,
  MmrInput,
  ModeProgress,
  Neighbour,
  OverviewProgress,
  PackDetail,
  PackInput,
  PackProgress,
  PackSort,
  PackStats,
  PairingInfo,
  ProfileRow,
  ProgressList,
  ProgressRange,
  ReplayStatus,
  ReplayToSave,
  SessionCard,
  SessionDetail,
  SessionReview,
  Settings,
  StatsApiStatus,
  ReportInfo,
} from "@shared/generated/types";

export class DesktopError extends Error {}

export function errorText(err: unknown): string {
  if (err instanceof Error) return err.message;
  if (typeof err === "string") return err;
  return "Something went wrong.";
}

/** What `update_settings` accepts; `install_dir: null` clears the manual folder. */
export interface SettingsPatch {
  phone_port?: number;
  session_timeout_min?: number;
  hard_hit_kmh?: number;
  install_dir?: string | null;
  replay_dir?: string | null;
  /** In-game name or account id; empty clears it. */
  me_hint?: string;
  setup_dismissed?: boolean;
  usage_ping?: boolean;
}

/** What the Feedback page hands to Rust; the screenshot is base64 without a data: prefix. */
export interface ReportInput {
  kind: "bug" | "idea";
  text: string;
  contact: string;
  screenshot: string | null;
  include_logs: boolean;
}

/** A newer release found by the updater. */
export interface UpdateInfo {
  version: string;
  notes: string | null;
}

export interface DesktopApi {
  snapshot(): Promise<AppSnapshot>;
  /** With `toSave`, only sessions with a ranked match whose replay is still to save. */
  sessions(search: string, offset: number, limit: number, toSave?: boolean): Promise<SessionCard[]>;
  sessionReview(id: number): Promise<SessionReview | null>;
  replaysToSave(): Promise<ReplayToSave[]>;
  replayStatus(): Promise<ReplayStatus>;
  /** Adds the newest replays of games that were never recorded; returns how many came in. */
  importPastGames(): Promise<number>;
  /** A match's replay page: every player, my usual in this mode, and what stood out. */
  matchReplay(matchId: number): Promise<MatchReplay>;
  reviewCount(): Promise<number>;
  session(id: number): Promise<SessionDetail | null>;
  setBlockPack(blockId: number, packId: number | null): Promise<void>;
  editReview(sessionId: number, mmr: MmrInput[], notes: string): Promise<void>;
  mergeSessions(sessionId: number, withSession: Neighbour): Promise<number>;
  splitSession(sessionId: number, afterBlockId: number): Promise<number>;
  deleteSession(sessionId: number): Promise<void>;
  /** True when the session went with its last activity. */
  deleteActivity(blockId: number): Promise<boolean>;
  packs(sort: PackSort, search: string): Promise<PackStats[]>;
  pack(id: number): Promise<PackDetail | null>;
  addPack(pack: PackInput): Promise<number>;
  updatePack(id: number, pack: PackInput): Promise<void>;
  setPackHidden(id: number, hidden: boolean): Promise<void>;
  settings(): Promise<Settings>;
  updateSettings(patch: SettingsPatch): Promise<Settings>;
  profiles(): Promise<ProfileRow[]>;
  setActiveProfile(primaryId: string): Promise<boolean>;
  pairing(): Promise<PairingInfo | null>;
  enablePhone(): Promise<PairingInfo>;
  disablePhone(): Promise<void>;
  regenerateToken(): Promise<PairingInfo | null>;
  fixStatsApi(): Promise<StatsApiStatus>;
  recheckStatsApi(): Promise<StatsApiStatus>;
  backups(): Promise<BackupInfo[]>;
  restoreBackup(name: string): Promise<void>;
  about(): Promise<About>;
  openDataFolder(): Promise<void>;
  openReleases(): Promise<void>;
  /** `null` where the window cannot control autostart (a browser during development). */
  autostart(): Promise<boolean | null>;
  setAutostart(on: boolean): Promise<void>;
  /** False where the window cannot quit or restart the app (a browser during development). */
  canControlApp: boolean;
  quitApp(): Promise<void>;
  restartApp(): Promise<void>;
  /** `null` when there is nothing newer, in a debug build, or outside the app. */
  checkUpdate(): Promise<UpdateInfo | null>;
  /** Downloads and installs; the app closes and starts again. */
  installUpdate(): Promise<void>;
  reportInfo(): Promise<ReportInfo>;
  sendReport(report: ReportInput): Promise<void>;
  /** Saves under Documents and shows the folder; returns its path. */
  saveReport(report: ReportInput): Promise<string>;
  progressList(range: ProgressRange): Promise<ProgressList>;
  progressOverview(range: ProgressRange): Promise<OverviewProgress>;
  progressMode(teamSize: number, range: ProgressRange): Promise<ModeProgress>;
  /** My last 20 ranked matches in a mode against their winners. */
  modeBenchmark(teamSize: number): Promise<Benchmark>;
  progressPack(packId: number, range: ProgressRange): Promise<PackProgress | null>;
}

export interface Transport {
  call(command: string, args: Record<string, unknown>): Promise<unknown>;
  autostart: { get(): Promise<boolean>; set(on: boolean): Promise<void> } | null;
  /** Quit and restart need the app's own window. */
  lifecycle?: { quit(): Promise<void>; restart(): Promise<void> } | null;
  updates?: { check(): Promise<UpdateInfo | null>; install(): Promise<void> } | null;
}

export interface TauriGlobal {
  core: { invoke(command: string, args?: Record<string, unknown>): Promise<unknown> };
  event: { listen(event: string, handler: (e: { payload: unknown }) => void): Promise<() => void> };
}

/** `window.__TAURI__` inside the app (`withGlobalTauri`), `null` in a browser. */
export function tauriGlobal(): TauriGlobal | null {
  return (window as unknown as { __TAURI__?: TauriGlobal }).__TAURI__ ?? null;
}

export function tauriTransport(tauri: TauriGlobal): Transport {
  return {
    call: (command, args) =>
      tauri.core.invoke("desktop", { command, args }).catch((err: unknown) => {
        throw new DesktopError(errorText(err));
      }),
    autostart: {
      get: () => tauri.core.invoke("get_autostart") as Promise<boolean>,
      set: async (on) => {
        await tauri.core.invoke("set_autostart", { enabled: on });
      },
    },
    lifecycle: {
      quit: async () => {
        await tauri.core.invoke("quit_app");
      },
      restart: async () => {
        await tauri.core.invoke("restart_app");
      },
    },
    updates: {
      check: () => tauri.core.invoke("check_update") as Promise<UpdateInfo | null>,
      install: async () => {
        await tauri.core.invoke("install_update");
      },
    },
  };
}

interface Envelope {
  ok: boolean;
  data?: unknown;
  error?: { message: string };
}

/** The loopback shim of `rl-replay serve --desktop-port` (dev server, end-to-end tests). */
export function httpTransport(fetchImpl: typeof fetch = fetch): Transport {
  return {
    async call(command, args) {
      let res: Response;
      try {
        res = await fetchImpl(`/desktop-api/${command}`, {
          method: "POST",
          headers: { "Content-Type": "application/json", "X-RL-Desktop": "1" },
          body: JSON.stringify(args),
        });
      } catch {
        throw new DesktopError("The app is not running.");
      }
      const json = (await res.json().catch(() => null)) as Envelope | null;
      if (!json || !json.ok) throw new DesktopError(json?.error?.message ?? `The app answered ${res.status}.`);
      return json.data;
    },
    autostart: null,
  };
}

export function createDesktopApi(t: Transport): DesktopApi {
  const call = <T>(command: string, args: Record<string, unknown> = {}) => t.call(command, args) as Promise<T>;
  const done = async (command: string, args: Record<string, unknown> = {}) => {
    await t.call(command, args);
  };
  return {
    snapshot: () => call("snapshot"),
    sessions: (search, offset, limit, toSave = false) =>
      call("sessions", toSave ? { search, offset, limit, to_save: true } : { search, offset, limit }),
    sessionReview: (id) => call("session_review", { id }),
    replaysToSave: () => call("replays_to_save"),
    replayStatus: () => call("replay_status"),
    importPastGames: () => call("import_past_games"),
    matchReplay: (matchId) => call("match_replay", { id: matchId }),
    reviewCount: () => call("review_count"),
    session: (id) => call("session", { id }),
    setBlockPack: (blockId, packId) => done("set_block_pack", { block_id: blockId, pack_id: packId }),
    editReview: (sessionId, mmr, notes) => done("edit_review", { session_id: sessionId, mmr, notes }),
    mergeSessions: (sessionId, withSession) => call("merge_sessions", { session_id: sessionId, with: withSession }),
    splitSession: (sessionId, afterBlockId) =>
      call("split_session", { session_id: sessionId, after_block_id: afterBlockId }),
    deleteSession: (sessionId) => done("delete_session", { session_id: sessionId }),
    deleteActivity: (blockId) => call("delete_activity", { block_id: blockId }),
    packs: (sort, search) => call("packs", { sort, search }),
    pack: (id) => call("pack", { id }),
    addPack: (pack) => call("add_pack", { pack }),
    updatePack: (id, pack) => done("update_pack", { id, pack }),
    setPackHidden: (id, hidden) => done("set_pack_hidden", { id, hidden }),
    settings: () => call("settings"),
    updateSettings: (patch) => call("update_settings", { patch }),
    profiles: () => call("profiles"),
    setActiveProfile: (primaryId) => call("set_active_profile", { primary_id: primaryId }),
    pairing: () => call("pairing"),
    enablePhone: () => call("enable_phone"),
    disablePhone: () => done("disable_phone"),
    regenerateToken: () => call("regenerate_token"),
    fixStatsApi: () => call("fix_stats_api"),
    recheckStatsApi: () => call("recheck_stats_api"),
    backups: () => call("backups"),
    restoreBackup: (name) => done("restore_backup", { name }),
    about: () => call("about"),
    openDataFolder: () => done("open_data_folder"),
    openReleases: () => done("open_releases"),
    autostart: () => (t.autostart ? t.autostart.get() : Promise.resolve(null)),
    setAutostart: (on) => (t.autostart ? t.autostart.set(on) : Promise.resolve()),
    canControlApp: t.lifecycle != null,
    quitApp: () => (t.lifecycle ? t.lifecycle.quit() : Promise.resolve()),
    restartApp: () => (t.lifecycle ? t.lifecycle.restart() : Promise.resolve()),
    checkUpdate: () => (t.updates ? t.updates.check() : Promise.resolve(null)),
    installUpdate: () => (t.updates ? t.updates.install() : Promise.resolve()),
    reportInfo: () => call("report_info"),
    sendReport: (report) => done("send_report", { ...report }),
    saveReport: (report) => call("save_report", { ...report }),
    progressList: (range) => call("progress_list", { range }),
    progressOverview: (range) => call("progress_overview", { range }),
    progressMode: (teamSize, range) => call("progress_mode", { team_size: teamSize, range }),
    modeBenchmark: (teamSize) => call("mode_benchmark", { team_size: teamSize }),
    progressPack: (packId, range) => call("progress_pack", { pack_id: packId, range }),
  };
}
