import { vi } from "vitest";
import type { Api } from "@shared/api/client";
import type { LiveSource, LiveState } from "@shared/api/liveStore";
import { fixtures } from "@shared/fixtures";
import { desktopFixtures } from "../desktop/fixtures";
import type { AwakeState } from "@shared/keepAwake";
import { searchPacks } from "./packSearch";

export function staticSource(state: LiveState): LiveSource {
  return { getState: () => state, subscribe: () => () => {} };
}

/** A source whose state the test can change; subscribers are notified like the real store. */
export function mutableSource(initial: LiveState) {
  let state = initial;
  const listeners = new Set<() => void>();
  return {
    getState: () => state,
    subscribe: (l: () => void) => {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },
    set(next: LiveState) {
      state = next;
      listeners.forEach((l) => l());
    },
  };
}

export function fakeApi(overrides: Partial<Api> = {}): Api {
  return {
    packs: vi.fn((search?: string) =>
      Promise.resolve(searchPacks(fixtures.packs, search)),
    ),
    setPack: vi.fn().mockResolvedValue(undefined),
    setSkipped: vi.fn().mockResolvedValue(undefined),
    reviewSession: vi.fn().mockResolvedValue(fixtures.idle.live.last_session),
    sessionReview: vi.fn().mockResolvedValue(desktopFixtures.review),
    ...overrides,
  };
}

/** A keep-awake control that turns on when asked, like the real one after a tap. */
export function fakeKeepAwake() {
  let state: AwakeState = "off";
  const listeners = new Set<() => void>();
  return {
    getState: () => state,
    subscribe: (l: () => void) => {
      listeners.add(l);
      return () => {
        listeners.delete(l);
      };
    },
    enable: vi.fn(async () => {
      state = "on";
      listeners.forEach((l) => l());
    }),
  };
}
