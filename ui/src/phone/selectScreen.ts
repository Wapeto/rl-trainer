import type { LiveState } from "@shared/api/liveStore";
import type { AppSnapshot, SessionSummary } from "@shared/generated/types";

export type Screen = "pairing" | "connecting" | "training" | "freeplay" | "match" | "session_end" | "idle";

/** The menus look the same as a queue or the pack browser, so the review waits this long. */
export const MENU_REVIEW_DELAY_MS = 15_000;

/** The session the review form is about: the open one while in the menus, else the last one. */
export function reviewTarget(snap: AppSnapshot): SessionSummary | null {
  return snap.live.session ? snap.live.current_session : snap.live.last_session;
}

/** Which screen the phone shows; recomputed from every snapshot and every second
 * (spec §11.2 order). */
export function selectScreen(state: LiveState, now: number): Screen {
  if (state.conn === "unpaired") return "pairing";
  const snap = state.snapshot;
  if (!snap) return "connecting";
  const block = snap.live.block;
  if (block) {
    if (block.kind === "training") return "training";
    if (block.kind === "freeplay") return "freeplay";
    if (block.kind === "match") return "match";
    return "idle";
  }
  const target = reviewTarget(snap);
  if (!target || target.reviewed) return "idle";
  if (!snap.live.session) return "session_end";
  // In the menus: ask for MMR only after ranked play, once the menus have lasted a while.
  const inMenus = now - target.ended_at >= MENU_REVIEW_DELAY_MS;
  return inMenus && target.ranked_team_sizes.length > 0 ? "session_end" : "idle";
}
