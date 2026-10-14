import { describe, expect, it } from "vitest";
import type { ReviewItem, SessionReview } from "@shared/generated/types";
import { desktopFixtures as f } from "../../desktop/fixtures";
import {
  coverageNote,
  describeItem,
  formatDelta,
  formatItemValue,
  summaryLine,
  toSaveParts,
  trainingBelow,
  trainingDetail,
  trainingRate,
} from "./format";

const item = (over: Partial<ReviewItem>): ReviewItem => ({
  key: "touches",
  label: "Touches",
  unit: "per_game",
  value: 98,
  usual: 78.8,
  better: true,
  spread: 6,
  covered: 1,
  of: 1,
  ...over,
});

describe("review wording", () => {
  it("formats values by unit", () => {
    expect(formatItemValue(item({ unit: "percent" }), 49.2)).toBe("49%");
    expect(formatItemValue(item({ unit: "per_game" }), 1.25)).toBe("1.3");
    expect(formatItemValue(item({ unit: "kmh" }), 81.4)).toBe("81 km/h");
  });

  it("words the difference as points, a relative change, or a plain difference", () => {
    expect(formatDelta(item({}))).toBe("▲ +24%");
    expect(formatDelta(item({ key: "supersonic", unit: "percent", value: 6, usual: 9 }))).toBe("▼ −3 pts");
    expect(formatDelta(item({ key: "shots", value: 4, usual: 9.4 }))).toBe("▼ −57%");
    expect(formatDelta(item({ key: "demos_given", value: 1.2, usual: 0.4 }))).toBe("▲ +0.8");
    expect(formatDelta(item({ key: "demos_given", value: 0.8, usual: 0 }))).toBe("▲ +0.8");
  });

  it("works the difference out from the numbers it shows", () => {
    expect(formatDelta(item({ unit: "percent", value: 49.4, usual: 30.6 }))).toBe("▲ +18 pts");
    expect(formatDelta(item({ key: "demos_given", value: 0.84, usual: 0.06 }))).toBe("▲ +0.7");
  });

  it("reads a row as a sentence and notes partial replay coverage", () => {
    expect(describeItem(item({}))).toBe("Touches 98, usual 78.8, 24% above");
    expect(describeItem(item({ label: "Supersonic", unit: "percent", value: 6, usual: 9 }))).toBe(
      "Supersonic 6%, usual 9%, 3 points below",
    );
    expect(coverageNote(item({ covered: 2, of: 3 }))).toBe("2 of 3 matches");
    expect(coverageNote(item({}))).toBeNull();
  });

  it("sums the review up in one line", () => {
    expect(summaryLine(f.review)).toBe("2 things stood out · 1 replay to save");
    const one: SessionReview = { ...f.review, modes: [{ team_size: 2, matches: 1, earlier: 9, items: [f.review.modes[1].items[0]] }], to_save: [] };
    expect(summaryLine(one)).toBe("1 thing stood out");
    const none: SessionReview = { ...f.review, modes: [{ team_size: 2, matches: 1, earlier: 9, items: [] }] };
    expect(summaryLine(none)).toBe("Nothing stood out against your usual · 1 replay to save");
    const early: SessionReview = { ...f.review, modes: [{ team_size: 1, matches: 1, earlier: 2, items: [] }], to_save: [] };
    expect(summaryLine(early)).toBe("Nothing to compare yet");
    expect(summaryLine({ ...f.review, modes: [], to_save: [] })).toBe("");
  });

  it("splits training into rate, target and details", () => {
    const t = f.review.training[0];
    expect(trainingRate(t)).toBe("20%");
    expect(trainingBelow(t)).toBe(true);
    expect(trainingDetail(t)).toBe("target 65% · Best stretch 8/20 after 7 min · dropped after 20 min · 10 quick resets left out");
    expect(trainingRate({ ...t, make_rate: null })).toBeNull();
  });

  it("splits a replay to save into result, mode and time", () => {
    const p = toSaveParts(f.review.to_save[0]);
    expect(p.result).toBe("L 3–8");
    expect(p.tone).toBe("loss");
    expect(p.mode).toBe("1v1");
    expect(p.time.length).toBeGreaterThan(0);
  });
});
