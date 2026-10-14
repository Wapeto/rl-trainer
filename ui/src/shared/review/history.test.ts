import { describe, expect, it } from "vitest";
import { HISTORY_SIZE, inHistory, SOON_GONE_PLACE } from "./history";

const DAY = 86_400_000;

describe("match history", () => {
  it("keeps the newest 20 matches for 30 days", () => {
    expect(HISTORY_SIZE).toBe(20);
    expect(SOON_GONE_PLACE).toBe(17);
    expect(inHistory(20, 0, 29 * DAY)).toBe(true);
    expect(inHistory(21, 0, DAY)).toBe(false);
    expect(inHistory(3, 0, 31 * DAY)).toBe(false);
  });
});
