import { describe, expect, it } from "vitest";
import { place } from "./place";

const tip = { width: 250, height: 80 };
const view = { width: 1000, height: 800 };

describe("place", () => {
  it("opens below the label when there is room", () => {
    expect(place({ left: 100, top: 100, bottom: 120 }, tip, view, "below")).toEqual({ left: 100, top: 128, side: "below" });
  });

  it("flips above near the bottom of the window", () => {
    expect(place({ left: 100, top: 740, bottom: 760 }, tip, view, "below")).toEqual({ left: 100, top: 652, side: "above" });
  });

  it("stays below when there is no room above either", () => {
    expect(place({ left: 100, top: 20, bottom: 760 }, tip, view, "below").side).toBe("below");
  });

  it("honours an above preference when it fits", () => {
    expect(place({ left: 100, top: 300, bottom: 320 }, tip, view, "above")).toEqual({ left: 100, top: 212, side: "above" });
  });

  it("shifts back inside the left and right edges", () => {
    expect(place({ left: 900, top: 100, bottom: 120 }, tip, view, "below").left).toBe(742);
    expect(place({ left: 2, top: 100, bottom: 120 }, tip, view, "below").left).toBe(8);
  });
});
