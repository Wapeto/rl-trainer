import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { BenchStat, Benchmark, PlayerReplayStats } from "@shared/generated/types";
import { fakeDesktopApi } from "../test/fakeApi";
import { WinnersCard } from "./WinnersCard";

const st = (gap: number, me = 40, winners_pct = 60): BenchStat => ({ gap, separates: Math.abs(gap) >= 0.3, me, winners_pct });
const full: Benchmark = {
  matches: 20,
  winners: { tracked_seconds: 300 } as PlayerReplayStats,
  losers: { tracked_seconds: 300 } as PlayerReplayStats,
  stats: {
    boost_collected: st(0.9), big_pads: st(0.5), small_pads: st(0.6),
    touches: st(0.1), pct_behind_ball: st(0.5),
  },
};

function renderCard(bench: Benchmark, teamSize = 2) {
  const api = fakeDesktopApi({ modeBenchmark: vi.fn().mockResolvedValue(bench) });
  render(<WinnersCard api={api} teamSize={teamSize} />);
  return api;
}

describe("WinnersCard", () => {
  it("draws one axis per group with the caption, loading for the team size", async () => {
    const api = renderCard(full);
    expect(await screen.findByText("Your last 20 ranked 2v2s against their winners.")).toBeInTheDocument();
    expect(api.modeBenchmark).toHaveBeenCalledWith(2);
    expect(screen.getByRole("figure", { name: "You against the winners, by category" })).toBeInTheDocument();
  });

  it("opens a group's radar from its button or axis, and closes it again", async () => {
    renderCard(full);
    const button = await screen.findByRole("button", { name: "Boost" });
    await userEvent.click(button);
    expect(button).toHaveAttribute("aria-pressed", "true");
    expect(screen.getByRole("figure", { name: "Boost: you against the winners" })).toBeInTheDocument();
    await userEvent.click(button);
    expect(screen.queryByRole("figure", { name: "Boost: you against the winners" })).toBeNull();
    await userEvent.click(screen.getAllByText("Boost").find((e) => e.tagName === "text")!);
    expect(screen.getByRole("figure", { name: "Boost: you against the winners" })).toBeInTheDocument();
  });

  it("opens a radar for a group with one separating stat, dimming the others", async () => {
    renderCard({ ...full, stats: { ...full.stats, big_pads: st(0.1), small_pads: st(0.1) } });
    await userEvent.click(await screen.findByRole("button", { name: "Boost" }));
    expect(screen.getByRole("figure", { name: "Boost: you against the winners" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Touches" }));
    expect(screen.getByText("Too few Touches stats measured to draw a radar.")).toBeInTheDocument();
  });

  it("says when too few groups are measured", async () => {
    renderCard({ ...full, stats: { boost_collected: st(0.9) } });
    expect(await screen.findByText("Too few stats measured in your last 20 ranked 2v2s.")).toBeInTheDocument();
  });

  it("shows a load error", async () => {
    const api = fakeDesktopApi({ modeBenchmark: vi.fn().mockRejectedValue(new Error("the database is locked")) });
    render(<WinnersCard api={api} teamSize={2} />);
    expect(await screen.findByText(/the database is locked/)).toBeInTheDocument();
  });

  it("explains when there are too few matches", async () => {
    renderCard({ matches: 7, winners: null, losers: null, stats: {} });
    expect(await screen.findByText("The winners' benchmark appears after 10 ranked replays in 2v2 (7 so far).")).toBeInTheDocument();
    expect(screen.queryByRole("figure")).toBeNull();
  });
});
