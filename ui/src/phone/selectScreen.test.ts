import { describe, expect, it } from "vitest";
import { FIXTURE_NOW, fixtures } from "@shared/fixtures";
import type { AppSnapshot } from "@shared/generated/types";
import type { LiveState } from "@shared/api/liveStore";
import { MENU_REVIEW_DELAY_MS, reviewTarget, selectScreen } from "./selectScreen";

const live = (snapshot: AppSnapshot) => ({ conn: "live" as const, snapshot });
const at = (state: LiveState, now = FIXTURE_NOW) => selectScreen(state, now);

describe("selectScreen", () => {
  it("asks for pairing first, then waits for the first snapshot", () => {
    expect(at({ conn: "unpaired", snapshot: fixtures.trainingBelow })).toBe("pairing");
    expect(at({ conn: "connecting", snapshot: null })).toBe("connecting");
  });

  it("shows the open activity", () => {
    expect(at(live(fixtures.trainingBelow))).toBe("training");
    expect(at(live(fixtures.freeplay))).toBe("freeplay");
    expect(at(live(fixtures.match))).toBe("match");
    const other = structuredClone(fixtures.freeplay);
    other.live.block!.kind = "other";
    expect(at(live(other))).toBe("idle");
  });

  it("asks for the review of a finished session until it is reviewed", () => {
    expect(at(live(fixtures.sessionEnd))).toBe("session_end");
    expect(at(live(fixtures.idle))).toBe("idle");
    expect(at(live(fixtures.idleEmpty))).toBe("idle");
  });

  it("asks for MMR after 15 s in the menus following ranked play, without a finished session", () => {
    const menuStart = fixtures.menus.live.current_session!.ended_at;
    expect(at(live(fixtures.menus), menuStart + MENU_REVIEW_DELAY_MS - 1)).toBe("idle");
    expect(at(live(fixtures.menus), menuStart + MENU_REVIEW_DELAY_MS)).toBe("session_end");
    expect(reviewTarget(fixtures.menus)?.id).toBe(12);
    expect(reviewTarget(fixtures.sessionEnd)?.id).toBe(11);
  });

  it("does not ask in the menus once reviewed, or without ranked play", () => {
    const reviewed = structuredClone(fixtures.menus);
    reviewed.live.current_session!.reviewed = true;
    expect(at(live(reviewed))).toBe("idle");
    const noRanked = structuredClone(fixtures.menus);
    noRanked.live.current_session!.ranked_team_sizes = [];
    expect(at(live(noRanked))).toBe("idle");
  });

  it("keeps the last screen while reconnecting", () => {
    expect(at({ conn: "reconnecting", snapshot: fixtures.match })).toBe("match");
  });
});
