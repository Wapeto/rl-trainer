import { describe, expect, it } from "vitest";
import {
  arenaName,
  clock,
  dayLabel,
  elapsed,
  hoursMinutes,
  kmh,
  longDuration,
  modeLabel,
  ratePercent,
  timeOfDay,
  whole,
} from "./format";

describe("format", () => {
  it("formats durations", () => {
    expect(elapsed(512_000)).toBe("8:32");
    expect(elapsed(3_909_000)).toBe("1:05:09");
    expect(elapsed(-5)).toBe("0:00");
    expect(hoursMinutes(3_900_000)).toBe("1:05");
    expect(longDuration(7_500_000)).toBe("2h 05");
    expect(longDuration(2_700_000)).toBe("45 min");
    expect(clock(167)).toBe("2:47");
  });

  it("formats numbers", () => {
    expect(whole(21.4)).toBe("21");
    expect(ratePercent(0.5833)).toBe("58");
    expect(ratePercent(null)).toBe("—");
    expect(kmh(112.4)).toBe("112");
  });

  it("names arenas and modes", () => {
    expect(arenaName("Stadium_P")).toBe("DFH Stadium");
    expect(arenaName("Mystery_Arena_P")).toBe("Mystery Arena");
    expect(modeLabel({ ranked: true, team_size: 2 })).toBe("Ranked 2v2");
    expect(modeLabel({ ranked: false, team_size: 3 })).toBe("Casual 3v3");
    expect(modeLabel({ ranked: false, team_size: null })).toBe("Match");
    expect(modeLabel({ ranked: false, team_size: 1, playlist_id: 6 })).toBe("Private 1v1");
  });

  it("labels days and times in local time", () => {
    const now = new Date(2026, 9, 8, 15, 0).getTime();
    expect(dayLabel(new Date(2026, 9, 8, 10, 0).getTime(), now)).toBe("Today");
    expect(dayLabel(new Date(2026, 9, 7, 22, 0).getTime(), now)).toBe("Yesterday");
    expect(dayLabel(new Date(2026, 9, 1, 12, 0).getTime(), now)).toBe("Thu 1 Oct");
    expect(timeOfDay(new Date(2026, 9, 8, 18, 10).getTime())).toBe("18:10");
  });
});
