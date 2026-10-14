import type { ReviewItem } from "@shared/generated/types";

/** Positions on the range track, each 0–100 % of its width. */
export interface Track {
  bandFrom: number;
  bandTo: number;
  usual: number;
  tick: number;
}

const SPREADS_SHOWN = 2;
const PAD = 0.08;

const clamp = (v: number, lo: number, hi: number) => Math.min(hi, Math.max(lo, v));

/** The usual ± spread as a band, the usual as a line, this session as a tick (spec §4.2). */
export function trackScale(item: ReviewItem): Track {
  const lo0 = Math.min(item.value, item.usual - SPREADS_SHOWN * item.spread);
  const hi0 = Math.max(item.value, item.usual + SPREADS_SHOWN * item.spread);
  const pad = (hi0 - lo0) * PAD;
  let lo = lo0 - pad;
  let hi = hi0 + pad;
  if (item.unit === "percent") {
    lo = Math.max(0, lo);
    hi = Math.min(100, hi);
  }
  const span = hi - lo || 1;
  const at = (v: number) => clamp(((v - lo) / span) * 100, 0, 100);
  return {
    bandFrom: at(item.usual - item.spread),
    bandTo: at(item.usual + item.spread),
    usual: at(item.usual),
    tick: at(item.value),
  };
}
