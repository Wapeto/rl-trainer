import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { FIXTURE_NOW, fixtures } from "@shared/fixtures";
import type { AppSnapshot } from "@shared/generated/types";
import { Match, bestText } from "./Match";

function renderMatch(snap: AppSnapshot = fixtures.match) {
  return render(<Match snap={snap} conn="live" now={FIXTURE_NOW} api={null} notify={vi.fn()} />);
}

describe("Match", () => {
  it("shows mode, arena, clock and the score", () => {
    renderMatch();
    expect(screen.getByText("Ranked 2v2")).toBeInTheDocument();
    expect(screen.getByText("Today W3 · L1")).toBeInTheDocument();
    expect(screen.getByRole("heading", { name: "DFH Stadium" })).toBeInTheDocument();
    expect(screen.getByText("2:47 left")).toBeInTheDocument();
    expect(screen.getByLabelText("Score 2 to 1")).toBeInTheDocument();
  });

  it("compares each stat with the best in the lobby", () => {
    renderMatch();
    expect(screen.getByText("best ✓").closest(".cmp__row")).toHaveClass("cmp__row--best");
    expect(screen.getByText("1 · tie")).toBeInTheDocument();
    expect(screen.getByText("5 · Vex")).toBeInTheDocument();
    expect(screen.getByText("515 · Kuro")).toBeInTheDocument();
    // Long names are cut to one line; the full text stays available.
    expect(screen.getByText("515 · Kuro")).toHaveAttribute("title", "515 · Kuro");
  });

  it("words the best column", () => {
    const row = { stat: "saves" as const, mine: 0, best: 0, best_name: null, tie: true, mine_is_best: false };
    expect(bestText(row)).toBe("—");
  });

  it("draws the four tiles as centered rings", () => {
    renderMatch();
    expect(screen.getByRole("img", { name: "Hardest shot 104 km/h" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Supersonic 21%" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Avg boost 38" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Time at 0 boost 14%" })).toBeInTheDocument();
  });

  it("shows overtime on the clock", () => {
    const ot = structuredClone(fixtures.match);
    ot.live.block!.match!.overtime = true;
    ot.live.block!.match!.clock_seconds = 42;
    renderMatch(ot);
    expect(screen.getByText("+0:42 OT")).toBeInTheDocument();
  });

  it("says it is waiting when the match data has not arrived", () => {
    const block = { ...fixtures.match.live.block!, match: null };
    renderMatch({ ...fixtures.match, live: { ...fixtures.match.live, block } });
    expect(screen.getByText("Waiting for match data")).toBeInTheDocument();
  });
});
