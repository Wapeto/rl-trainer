import { describe, expect, it } from "vitest";
import { desktopFixtures as f } from "../fixtures";
import { canSplit, cardSummary, detailSummary, previousPackId, rateTone, timelineItems } from "./timeline";

describe("timeline", () => {
  it("groups consecutive matches of the same mode", () => {
    const items = timelineItems(f.detail.activities);
    expect(items.map((i) => i.kind)).toEqual(["single", "single", "single", "matches"]);
    const group = items[3];
    expect(group.kind === "matches" && group.activities.map((a) => a.id)).toEqual([13, 14, 15]);
  });

  it("keeps private and casual matches of the same size apart", () => {
    const casual = f.detail.activities[3];
    const asMode = (id: number, playlist: number, ranked: boolean) => ({
      ...casual,
      id,
      playlist_id: playlist,
      match_summary: { ...casual.match_summary!, ranked, team_size: 1 },
    });
    const items = timelineItems([asMode(1, 6, false), asMode(2, 6, false), asMode(3, 1, false)]);
    expect(items.map((i) => i.kind === "matches" && i.activities.length)).toEqual([2, 1]);
    expect(items[0].kind === "matches" && items[0].playlistId).toBe(6);
  });

  it("offers a split only across a break of five minutes or more", () => {
    const a = f.detail.activities;
    expect(canSplit(a[1], a[2])).toBe(false); // one minute apart
    expect(canSplit(a[2], a[3])).toBe(true); // seven minutes apart
  });

  it("summarises a session card", () => {
    expect(f.cards.map(cardSummary)).toEqual(["2 packs · 3 ranked", "1 pack · 2 ranked", "Free play"]);
    expect(detailSummary(f.detail)).toBe("2 packs · 3 ranked");
  });

  it("suggests the pack of the previous training activity", () => {
    expect(previousPackId(f.detail.activities, 2)).toBe(11);
    expect(previousPackId(f.detail.activities, 1)).toBeNull();
  });

  it("colours a rate against its target", () => {
    expect(rateTone(0.66, 0.65)).toBe("green");
    expect(rateTone(0.52, 0.65)).toBe("amber");
    expect(rateTone(null, 0.65)).toBe("amber");
  });
});
