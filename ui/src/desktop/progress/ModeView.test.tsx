import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { PROGRESS_NOW } from "../fixtures";
import { fakeDesktopApi, memoryStorage } from "../test/fakeApi";
import { MODE_STATS, PROGRESS } from "@shared/glossary";
import { ModeView } from "./ModeView";

function renderView(storage = memoryStorage()) {
  const api = fakeDesktopApi();
  render(
    <ModeView
      api={api}
      snapshot={null}
      now={PROGRESS_NOW}
      notify={vi.fn()}
      reload="r"
      onChanged={vi.fn()}
      storage={storage}
      onOpenSession={vi.fn()}
      onOpenPack={vi.fn()}
      range="days90"
      teamSize={2}
    />,
  );
  return { api, storage };
}

describe("ModeView", () => {
  it("explains the tiles and every stat card on hover", async () => {
    renderView();
    const tile = await screen.findByText("Team share");
    await userEvent.hover(tile);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(PROGRESS.team_share.text);
    await userEvent.unhover(tile);
    await userEvent.hover(within(screen.getByRole("tabpanel")).getByRole("button", { name: "Assists" }));
    const tip = await screen.findByRole("tooltip");
    expect(tip).toHaveTextContent(MODE_STATS.assists.text);
    expect(tip).toHaveTextContent(/^Assists/);
  });

  it("shows MMR, record and the scoreboard tab first", async () => {
    const { api } = renderView();
    expect(await screen.findByRole("heading", { level: 1, name: "Ranked 2v2" })).toBeInTheDocument();
    expect(api.progressMode).toHaveBeenCalledWith(2, "days90");
    expect(screen.getByRole("img", { name: "MMR" })).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Win rate" })).toBeInTheDocument();
    expect(screen.getByRole("tab", { name: "Scoreboard" })).toHaveAttribute("aria-selected", "true");
    const focus = within(screen.getByRole("tabpanel")).getByRole("region", { name: "Selected stat" });
    expect(within(focus).getByRole("img", { name: "Goals" })).toBeInTheDocument();
  });

  it("switches tabs, remembers the tab, and says since when new stats exist", async () => {
    const { storage } = renderView();
    await userEvent.click(await screen.findByRole("tab", { name: "Movement" }));
    expect(storage.data["rl-progress-tab"]).toBe("movement");
    const focus = within(screen.getByRole("tabpanel")).getByRole("region", { name: "Selected stat" });
    expect(within(focus).getByRole("img", { name: "Airborne" })).toBeInTheDocument();
    expect(screen.getByText(/^since /)).toBeInTheDocument();
  });

  it("shows the chosen stat large above the tab's small charts, and remembers it per tab", async () => {
    const { storage } = renderView(memoryStorage({ "rl-progress-tab": "boost" }));
    const panel = await screen.findByRole("tabpanel");
    const small = within(panel).getAllByRole("button", { pressed: false }).length + 1;
    expect(within(panel).getAllByRole("button")).toHaveLength(small);
    expect(within(panel).getByRole("button", { name: "Average boost" })).toHaveAttribute("aria-pressed", "true");
    await userEvent.click(within(panel).getByRole("button", { name: "Big pads" }));
    const focus = within(panel).getByRole("region", { name: "Selected stat" });
    expect(within(focus).getByRole("img", { name: "Big pads" })).toBeInTheDocument();
    expect(within(focus).getByText("estimated")).toBeInTheDocument();
    expect(within(panel).getByRole("button", { name: "Big pads" })).toHaveAttribute("aria-pressed", "true");
    expect(storage.data["rl-progress-focus-boost"]).toBe("big_pads");
    expect(screen.queryByRole("dialog")).toBeNull();
  });
});
