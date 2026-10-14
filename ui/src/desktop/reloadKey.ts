import type { AppSnapshot } from "@shared/generated/types";

/** Changes when what the pages show may have changed: a session opened or closed, an activity
 * started or ended, a review was saved, or the engine restarted (a restore). */
export function reloadKey(snap: AppSnapshot | null): string {
  if (!snap) return "none";
  const l = snap.live;
  return [
    snap.generation,
    snap.replays,
    l.session?.id ?? "-",
    l.block?.id ?? "-",
    l.last_session?.id ?? "-",
    l.last_session?.reviewed ? 1 : 0,
    l.current_session?.ended_at ?? "-",
  ].join(":");
}
