import type { MmrInput, PackStats, SessionReview, SessionSummary } from "@shared/generated/types";

export class ApiError extends Error {
  constructor(
    readonly code: string,
    message: string,
    readonly status: number,
  ) {
    super(message);
  }
}

export interface Api {
  packs(search?: string): Promise<PackStats[]>;
  setPack(blockId: number, packId: number | null): Promise<void>;
  setSkipped(attemptId: number, skipped: boolean): Promise<void>;
  reviewSession(sessionId: number, mmr: MmrInput[], notes: string | null): Promise<SessionSummary>;
  /** The after-session review (null for an unknown session). */
  sessionReview(sessionId: number): Promise<SessionReview | null>;
}

interface Envelope<T> {
  ok: boolean;
  data?: T;
  error?: { code: string; message: string };
}

export function createApi(token: string, fetchImpl: typeof fetch = fetch, onUnauthorized?: () => void): Api {
  async function call<T>(method: string, path: string, body?: unknown): Promise<T> {
    let res: Response;
    try {
      res = await fetchImpl(path, {
        method,
        headers: { Authorization: `Bearer ${token}`, "Content-Type": "application/json" },
        body: body === undefined ? undefined : JSON.stringify(body),
      });
    } catch {
      throw new ApiError("network", "Can't reach the PC. Check the Wi-Fi.", 0);
    }
    const json = (await res.json().catch(() => null)) as Envelope<T> | null;
    if (!json || !json.ok) {
      // A refused token means this phone must pair again (the PC made a new code).
      if (res.status === 401) onUnauthorized?.();
      throw new ApiError(
        json?.error?.code ?? "http",
        json?.error?.message ?? `The PC answered ${res.status}.`,
        res.status,
      );
    }
    return json.data as T;
  }
  return {
    packs: (search = "") =>
      call<PackStats[]>("GET", search.trim() ? `/api/packs?search=${encodeURIComponent(search.trim())}` : "/api/packs"),
    setPack: async (blockId, packId) => {
      await call("PUT", `/api/blocks/${blockId}/pack`, { pack_id: packId });
    },
    setSkipped: async (attemptId, skipped) => {
      await call("PUT", `/api/attempts/${attemptId}/skipped`, { skipped });
    },
    reviewSession: (sessionId, mmr, notes) =>
      call<SessionSummary>("POST", `/api/sessions/${sessionId}/review`, { mmr, notes }),
    sessionReview: (sessionId) => call<SessionReview | null>("GET", `/api/sessions/${sessionId}/review`),
  };
}
