import { describe, expect, it } from "vitest";
import { STATS } from "../desktop/progress/statMeta";
import {
  glossFor,
  lowBoostGloss,
  MODE_STATS,
  noReplayGloss,
  placeGloss,
  PROGRESS,
  REPLAY_STATS,
  statGloss,
  TERMS,
  usualGloss,
} from "./glossary";

const REVIEW_REPLAY_KEYS = ["low_boost_touches", "behind_ball", "first_man_share", "fifty_wins", "whiffs"];

describe("glossary", () => {
  it("explains every stat the coach can pick", () => {
    for (const key of [...Object.keys(STATS), ...REVIEW_REPLAY_KEYS]) {
      const gloss = glossFor(key);
      expect(gloss, key).not.toBeNull();
      expect(gloss!.title.length, key).toBeGreaterThan(0);
      expect(gloss!.text.endsWith("."), key).toBe(true);
    }
  });

  it("has no hint for an unknown key, even an object property name", () => {
    expect(glossFor("nope")).toBeNull();
    expect(glossFor("constructor")).toBeNull();
    expect(glossFor("toString")).toBeNull();
  });

  it("explains a whiff and the tile-only replay stats", () => {
    expect(REPLAY_STATS.whiffs.text).toMatch(/missed/);
    expect(REPLAY_STATS.kickoffs.title).toBe("Kickoffs won–lost");
    expect(REPLAY_STATS.first_second_man.title).toBe("1st / 2nd man");
    expect(MODE_STATS.boost_per_minute.text).toMatch(/Estimated/);
  });

  it("explains every Progress and Packs tile", () => {
    for (const [key, gloss] of Object.entries(PROGRESS)) {
      expect(gloss.title.length, key).toBeGreaterThan(0);
      expect(gloss.text.endsWith("."), key).toBe(true);
    }
    expect(PROGRESS.best_session.text).toMatch(/10 attempts/);
  });

  it("titles a stat hint like the card that shows it", () => {
    expect(statGloss("defensive_third", "Touches in defensive third")).toEqual({
      title: "Touches in defensive third",
      text: MODE_STATS.defensive_third.text,
    });
  });

  it("fills in values", () => {
    expect(usualGloss("2v2").text).toMatch(/earlier ranked 2v2 matches/);
    expect(placeGloss(19).text).toMatch(/about place 19 of 20/);
    expect(noReplayGloss(12).text).toMatch(/^About place 12 of 20/);
    expect(lowBoostGloss(17, 36).text).toMatch(/17 of your 36 touches\.$/);
    expect(TERMS.to_save.title).toBe("Replays to save");
  });
});
