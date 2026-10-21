import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { ActivityRow, PlayerReplayStats } from "@shared/generated/types";
import { desktopFixtures as f } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { MatchPage } from "./MatchPage";
import { KEY_STATS, STAT_GROUPS, bestValue, statValue } from "./replayRows";

// The fixture match with a replay is a 2v2 (ts_export samples use team_size 2).
const twos = f.detail.activities.find((a) => a.match_summary?.replay)!;
const ones: ActivityRow = { ...twos, match_summary: { ...twos.match_summary!, team_size: 1 } };
const row = (key: string) => [KEY_STATS, ...STAT_GROUPS].flatMap((g) => g.rows).find((r) => r.key === key)!;

describe("MatchPage", () => {
  it("lists what stood out, and a click highlights the row", async () => {
    render(<MatchPage api={fakeDesktopApi()} activity={twos} onBack={vi.fn()} />);
    const sum = await screen.findByRole("region", { name: "What stood out" });
    const first = within(sum).getAllByRole("button")[0];
    expect(first).toHaveTextContent("Time at 0 boost: 26% — winners average 13%.");
    await userEvent.click(first);
    const key = screen.getByRole("table", { name: "Key stats" });
    expect(within(key).getByRole("row", { name: /Time at 0 boost/ })).toHaveClass("d-ps__flash");
  });

  it("shows me, the winners, my team in blue, then the opponents in orange", async () => {
    render(<MatchPage api={fakeDesktopApi()} activity={twos} onBack={vi.fn()} />);
    const key = await screen.findByRole("table", { name: "Key stats" });
    expect(within(key).getAllByRole("columnheader").map((h) => h.textContent)).toEqual(["", "You", "Winners", "Teammate", "Rival A", "Rival B"]);
    expect(within(key).getByRole("columnheader", { name: "Teammate" })).toHaveClass("d-ps__us");
    expect(within(key).getByRole("columnheader", { name: "Rival A" })).toHaveClass("d-ps__them", "d-ps__split");
  });

  it("shows counts per 5 minutes with the match total on hover", async () => {
    render(<MatchPage api={fakeDesktopApi()} activity={twos} onBack={vi.fn()} />);
    const key = await screen.findByRole("table", { name: "Key stats" });
    const cells = within(within(key).getByRole("row", { name: /Whiffs/ })).getAllByRole("cell");
    expect(cells.map((c) => c.textContent)).toEqual(["3.0", "2.0", "2.0", "5.0", "—"]);
    expect(cells[0]).toHaveAttribute("title", "3 in the match");
  });

  it("folds the other groups and opens one from the index", async () => {
    render(<MatchPage api={fakeDesktopApi()} activity={twos} onBack={vi.fn()} />);
    await screen.findByRole("table", { name: "Key stats" });
    expect(screen.queryByRole("table", { name: "Boost" })).toBeNull();
    const index = screen.getByRole("navigation", { name: "Categories" });
    await userEvent.click(within(index).getByRole("button", { name: "Boost" }));
    expect(screen.getByRole("table", { name: "Boost" })).toBeInTheDocument();
    expect(within(index).getByRole("button", { name: "Boost" })).toHaveAttribute("aria-current", "true");
  });

  it("hides team-only rows in 1v1", async () => {
    render(<MatchPage api={fakeDesktopApi()} activity={ones} onBack={vi.fn()} />);
    const key = await screen.findByRole("table", { name: "Key stats" });
    expect(within(key).queryByRole("row", { name: /1st man/ })).toBeNull();
    const index = screen.getByRole("navigation", { name: "Categories" });
    expect(within(index).queryByRole("button", { name: "Rotation" })).toBeNull();
    expect(within(index).queryByRole("button", { name: "Passing" })).toBeNull();
  });

  it("says how many ranked replays the benchmark still needs", async () => {
    const api = fakeDesktopApi({
      matchReplay: vi.fn().mockResolvedValue({ ...f.matchReplay, benchmark: { matches: 7, winners: null, losers: null, stats: {} } }),
    });
    render(<MatchPage api={api} activity={twos} onBack={vi.fn()} />);
    expect(await screen.findByText("The winners' benchmark appears after 10 ranked replays in 2v2 (7 so far).")).toBeInTheDocument();
    expect(within(screen.getByRole("table", { name: "Key stats" })).queryByRole("columnheader", { name: "Winners" })).toBeNull();
    expect(screen.queryByRole("figure")).toBeNull();
  });

  it("shows the overview radar beside what stood out", async () => {
    render(<MatchPage api={fakeDesktopApi()} activity={twos} onBack={vi.fn()} />);
    const sum = await screen.findByRole("region", { name: "What stood out" });
    const fig = within(sum).getByRole("figure", { name: "You against the winners, by category" });
    expect(within(fig).getAllByRole("listitem").map((li) => li.textContent?.split(":")[0])).toEqual(["Positioning", "Boost", "Touches", "Movement"]);
    expect(within(sum).getByText("Against the winners of your last 20 ranked 2v2s.")).toBeInTheDocument();
  });

  it("no overview under 3 groups, but the group radar still shows", async () => {
    // Only Boost and Touches keep a separating stat.
    const keep = ["boost_collected", "big_pads", "pct_zero_boost", "boost_used_supersonic", "touches", "hard_hits"];
    const stats = Object.fromEntries(Object.entries(f.matchReplay.benchmark.stats).filter(([k]) => keep.includes(k)));
    const api = fakeDesktopApi({ matchReplay: vi.fn().mockResolvedValue({ ...f.matchReplay, benchmark: { ...f.matchReplay.benchmark, stats } }) });
    render(<MatchPage api={api} activity={twos} onBack={vi.fn()} />);
    const sum = await screen.findByRole("region", { name: "What stood out" });
    expect(within(sum).queryByRole("figure")).toBeNull();
    await userEvent.click(within(screen.getByRole("navigation", { name: "Categories" })).getByRole("button", { name: "Boost" }));
    expect(screen.getByRole("figure", { name: "Boost: you against the winners" })).toBeInTheDocument();
  });

  it("explains a group without a radar, and marks the better way on separating rows", async () => {
    render(<MatchPage api={fakeDesktopApi()} activity={twos} onBack={vi.fn()} />);
    await screen.findByRole("table", { name: "Key stats" });
    await userEvent.click(within(screen.getByRole("navigation", { name: "Categories" })).getByRole("button", { name: "Positioning" }));
    expect(screen.getByText("Nothing in Positioning separated winners from losers in your last 20 matches.")).toBeInTheDocument();
    const table = screen.getByRole("table", { name: "Positioning" });
    expect(within(within(table).getByRole("row", { name: /Behind the ball/ })).getByLabelText("more is better")).toBeInTheDocument();
  });

  it("says when nothing stood out", async () => {
    const api = fakeDesktopApi({ matchReplay: vi.fn().mockResolvedValue({ ...f.matchReplay, insights: [] }) });
    render(<MatchPage api={api} activity={twos} onBack={vi.fn()} />);
    expect(await screen.findByText("Nothing stood out in this match.")).toBeInTheDocument();
  });

  it("falls back to my stats when the players were not read", async () => {
    const api = fakeDesktopApi({ matchReplay: vi.fn().mockResolvedValue({ ...f.matchReplay, players: [] }) });
    render(<MatchPage api={api} activity={twos} onBack={vi.fn()} />);
    expect(await screen.findByText("Insights appear once this replay has been read for every player.")).toBeInTheDocument();
    expect(within(screen.getByRole("table", { name: "Key stats" })).getByRole("row", { name: /In front of the ball/ })).toHaveTextContent("23%");
  });

  it("falls back to my stats on an error", async () => {
    const api = fakeDesktopApi({ matchReplay: vi.fn().mockRejectedValue(new Error("database locked")) });
    render(<MatchPage api={api} activity={twos} onBack={vi.fn()} />);
    expect(await screen.findByText("database locked")).toBeInTheDocument();
    expect(within(screen.getByRole("table", { name: "Key stats" })).getByRole("row", { name: /Whiffs/ })).toHaveTextContent(/\d/);
  });

  it("goes back", async () => {
    const onBack = vi.fn();
    render(<MatchPage api={fakeDesktopApi()} activity={twos} onBack={onBack} />);
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(onBack).toHaveBeenCalledTimes(1);
  });
});

describe("statValue", () => {
  const s = { tracked_seconds: 600, whiffs: 4, touches: 20, touches_low_boost: 5, secs_first_man: 150, goals: 2 } as PlayerReplayStats;
  it("turns counts into per 5 minutes, seconds into shares, and low touches into a share of touches", () => {
    expect(statValue(row("whiffs"), s)).toBe(2);
    expect(statValue(row("secs_first_man"), s)).toBe(25);
    expect(statValue(row("touches_low_boost"), s)).toBe(25);
    expect(statValue(row("goals"), s)).toBe(2);
    expect(statValue(row("whiffs"), { ...s, tracked_seconds: null })).toBeNull();
  });
});

describe("bestValue", () => {
  it("picks the highest or lowest, and nothing when all are equal or only one is known", () => {
    expect(bestValue([1, 3, null], "high")).toBe(3);
    expect(bestValue([1, 3, null], "low")).toBe(1);
    expect(bestValue([2, 2], "high")).toBeNull();
    expect(bestValue([2, null], "high")).toBeNull();
  });
});
