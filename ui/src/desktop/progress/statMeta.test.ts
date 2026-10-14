import { describe, expect, it } from "vitest";
import { desktopFixtures as f } from "../fixtures";
import { formatStat, GROUPS, STATS } from "./statMeta";

describe("stat meta", () => {
  it("describes every stat the app sends, in one of the six tabs (win rate on top)", () => {
    for (const s of f.progressMode.stats) {
      expect(STATS[s.stat]).toBeDefined();
    }
    const grouped = Object.values(STATS).filter((m) => m.group != null);
    expect(new Set(grouped.map((m) => m.group))).toEqual(new Set(GROUPS.map((g) => g.id)));
    expect(STATS.win_rate.group).toBeNull();
  });

  it("formats values with their unit", () => {
    expect(formatStat("shooting_pct", 31.6)).toBe("32%");
    expect(formatStat("goals", 0.86)).toBe("0.9");
    expect(formatStat("goal_speed", 94.2)).toBe("94 km/h");
    expect(formatStat("goals", null)).toBe("—");
  });
});
