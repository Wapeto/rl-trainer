import type { ActivityRow, SessionCard, SessionDetail } from "@shared/generated/types";

/** The pack table's default target, used for training without a pack. */
export const DEFAULT_TARGET = 0.65;

export type TimelineItem =
  | { kind: "single"; activity: ActivityRow }
  | { kind: "matches"; activities: ActivityRow[]; ranked: boolean; teamSize: number | null; playlistId: number | null };

/** "Split here" is offered only across a break at least this long. */
export const SPLIT_MIN_GAP_MS = 5 * 60_000;

/** Whether the break between two finished activities is long enough to offer a split. */
export function canSplit(before: ActivityRow, after: ActivityRow): boolean {
  return before.ended_at != null && after.started_at - before.ended_at >= SPLIT_MIN_GAP_MS;
}

/** One card per activity, except consecutive matches of the same mode (and playlist, so private
 * and casual games stay apart), which share one. */
export function timelineItems(activities: ActivityRow[]): TimelineItem[] {
  return activities.reduce<TimelineItem[]>((items, a) => {
    const m = a.match_summary;
    const last = items[items.length - 1];
    if (
      m &&
      last?.kind === "matches" &&
      last.ranked === m.ranked &&
      last.teamSize === m.team_size &&
      last.playlistId === a.playlist_id
    ) {
      return [...items.slice(0, -1), { ...last, activities: [...last.activities, a] }];
    }
    if (m) {
      return [...items, { kind: "matches", activities: [a], ranked: m.ranked, teamSize: m.team_size, playlistId: a.playlist_id }];
    }
    return [...items, { kind: "single", activity: a }];
  }, []);
}

export const itemFirst = (item: TimelineItem): ActivityRow =>
  item.kind === "single" ? item.activity : item.activities[0];

export const itemLast = (item: TimelineItem): ActivityRow =>
  item.kind === "single" ? item.activity : item.activities[item.activities.length - 1];

type Counts = Pick<SessionCard, "packs" | "ranked_matches" | "casual_matches" | "freeplay_activities">;

/** "2 packs · 3 ranked", "Free play", or "No activities". */
export function cardSummary(c: Counts): string {
  const parts: string[] = [];
  if (c.packs > 0) parts.push(`${c.packs} ${c.packs === 1 ? "pack" : "packs"}`);
  if (c.ranked_matches > 0) parts.push(`${c.ranked_matches} ranked`);
  if (c.casual_matches > 0) parts.push(`${c.casual_matches} casual`);
  if (parts.length === 0 && c.freeplay_activities > 0) parts.push("Free play");
  return parts.length > 0 ? parts.join(" · ") : "No activities";
}

/** The same summary, built from a loaded session. */
export function detailSummary(d: SessionDetail): string {
  const training = d.activities.filter((a) => a.kind === "training");
  const matches = d.activities.flatMap((a) => (a.match_summary ? [a.match_summary] : []));
  return cardSummary({
    packs: new Set(training.map((a) => a.pack?.id ?? -1)).size,
    ranked_matches: matches.filter((m) => m.ranked).length,
    casual_matches: matches.filter((m) => !m.ranked).length,
    freeplay_activities: d.activities.filter((a) => a.kind === "freeplay").length,
  });
}

/** The pack of the nearest earlier training activity that has one. */
export function previousPackId(activities: ActivityRow[], index: number): number | null {
  for (let i = index - 1; i >= 0; i--) {
    const pack = activities[i].pack;
    if (activities[i].kind === "training" && pack) return pack.id;
  }
  return null;
}

export function rateTone(rate: number | null, target: number): "green" | "amber" {
  return rate != null && rate >= target ? "green" : "amber";
}
