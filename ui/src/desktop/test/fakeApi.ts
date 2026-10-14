import { vi } from "vitest";
import type { AppSnapshot, PackSort } from "@shared/generated/types";
import { searchPacks } from "../../test/packSearch";
import type { DesktopApi } from "../api";
import { desktopFixtures as f } from "../fixtures";
import type { SnapshotFeed } from "../live";

const okStats = { state: "ok", rate: 10, port: 49123, web_port: 49124 } as const;

export function fakeDesktopApi(overrides: Partial<DesktopApi> = {}): DesktopApi {
  return {
    snapshot: vi.fn().mockRejectedValue(new Error("not used")),
    sessions: vi.fn().mockResolvedValue(f.cards),
    sessionReview: vi.fn().mockResolvedValue(f.review),
    replaysToSave: vi.fn().mockResolvedValue(f.toSave),
    replayStatus: vi.fn().mockResolvedValue(f.replayStatus),
    matchReplay: vi.fn().mockResolvedValue(f.matchReplay),
    reviewCount: vi.fn().mockResolvedValue(1),
    session: vi.fn().mockResolvedValue(f.detail),
    setBlockPack: vi.fn().mockResolvedValue(undefined),
    editReview: vi.fn().mockResolvedValue(undefined),
    mergeSessions: vi.fn().mockResolvedValue(11),
    splitSession: vi.fn().mockResolvedValue(13),
    deleteSession: vi.fn().mockResolvedValue(undefined),
    deleteActivity: vi.fn().mockResolvedValue(false),
    packs: vi.fn((_sort: PackSort, search: string) => Promise.resolve(searchPacks(f.packList, search))),
    pack: vi.fn().mockResolvedValue(f.packDetail),
    addPack: vi.fn().mockResolvedValue(42),
    updatePack: vi.fn().mockResolvedValue(undefined),
    setPackHidden: vi.fn().mockResolvedValue(undefined),
    settings: vi.fn().mockResolvedValue(f.settings),
    updateSettings: vi.fn().mockResolvedValue(f.settings),
    profiles: vi.fn().mockResolvedValue(f.profiles),
    setActiveProfile: vi.fn().mockResolvedValue(true),
    pairing: vi.fn().mockResolvedValue(f.pairing),
    enablePhone: vi.fn().mockResolvedValue(f.pairing),
    disablePhone: vi.fn().mockResolvedValue(undefined),
    regenerateToken: vi.fn().mockResolvedValue(f.pairing),
    fixStatsApi: vi.fn().mockResolvedValue(okStats),
    recheckStatsApi: vi.fn().mockResolvedValue(okStats),
    backups: vi.fn().mockResolvedValue(f.backups),
    restoreBackup: vi.fn().mockResolvedValue(undefined),
    about: vi.fn().mockResolvedValue(f.about),
    openDataFolder: vi.fn().mockResolvedValue(undefined),
    openReleases: vi.fn().mockResolvedValue(undefined),
    autostart: vi.fn().mockResolvedValue(true),
    setAutostart: vi.fn().mockResolvedValue(undefined),
    canControlApp: true,
    quitApp: vi.fn().mockResolvedValue(undefined),
    restartApp: vi.fn().mockResolvedValue(undefined),
    checkUpdate: vi.fn().mockResolvedValue(null),
    installUpdate: vi.fn().mockResolvedValue(undefined),
    reportInfo: vi.fn().mockResolvedValue({ can_send: true }),
    sendReport: vi.fn().mockResolvedValue(undefined),
    saveReport: vi.fn().mockResolvedValue("C:/Docs/RL Trainer reports/x"),
    progressList: vi.fn().mockResolvedValue(f.progressList),
    progressOverview: vi.fn().mockResolvedValue(f.progressOverview),
    progressMode: vi.fn().mockResolvedValue(f.progressMode),
    progressPack: vi.fn().mockResolvedValue(f.progressPack),
    modeBenchmark: vi.fn().mockResolvedValue(f.matchReplay.benchmark),
    ...overrides,
  };
}

export function feedOf(snapshot: AppSnapshot | null): SnapshotFeed {
  return { getState: () => snapshot, subscribe: () => () => {} };
}

export function memoryStorage(initial: Record<string, string> = {}) {
  const data = { ...initial };
  return {
    data,
    getItem: (k: string) => data[k] ?? null,
    setItem: (k: string, v: string) => {
      data[k] = v;
    },
  };
}
