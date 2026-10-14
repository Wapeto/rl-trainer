export type Side = "below" | "above";

interface Anchor {
  left: number;
  top: number;
  bottom: number;
}

interface Size {
  width: number;
  height: number;
}

const GAP = 8;
const MARGIN = 8;

/** Where a hint goes: next to its label, flipped when it does not fit, kept inside the window. */
export function place(anchor: Anchor, tip: Size, view: Size, prefer: Side): { left: number; top: number; side: Side } {
  const below = anchor.bottom + GAP;
  const above = anchor.top - GAP - tip.height;
  const fitsBelow = below + tip.height <= view.height - MARGIN;
  const fitsAbove = above >= MARGIN;
  const side: Side =
    prefer === "below" ? (fitsBelow || !fitsAbove ? "below" : "above") : fitsAbove || !fitsBelow ? "above" : "below";
  const maxLeft = Math.max(MARGIN, view.width - MARGIN - tip.width);
  const left = Math.min(Math.max(anchor.left, MARGIN), maxLeft);
  return { left, top: side === "below" ? below : above, side };
}
