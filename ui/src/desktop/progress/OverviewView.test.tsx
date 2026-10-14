import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { desktopFixtures as f, PROGRESS_NOW } from "../fixtures";
import { fakeDesktopApi, memoryStorage } from "../test/fakeApi";
import { PROGRESS } from "@shared/glossary";
import { OverviewView } from "./OverviewView";

function renderView(overview = f.progressOverview) {
  const onSelect = vi.fn();
  const onOpenSession = vi.fn();
  const api = fakeDesktopApi({ progressOverview: vi.fn().mockResolvedValue(overview) });
  render(
    <OverviewView
      api={api}
      snapshot={null}
      now={PROGRESS_NOW}
      notify={vi.fn()}
      reload="r"
      onChanged={vi.fn()}
      storage={memoryStorage()}
      onOpenSession={onOpenSession}
      onOpenPack={vi.fn()}
      range="days90"
      onSelect={onSelect}
    />,
  );
  return { api, onSelect, onOpenSession };
}

describe("OverviewView", () => {
  it("explains the summary tiles on hover", async () => {
    renderView();
    const tiles = (await screen.findByText("Packs at target")).closest(".d-kpis") as HTMLElement;
    await userEvent.hover(within(tiles).getByText("Packs at target"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent(PROGRESS.packs_at_target.text);
  });

  it("shows the four cards, MMR per mode, the movers and the weeks", async () => {
    const { api } = renderView();
    expect(await screen.findByRole("heading", { level: 1, name: "Overview" })).toBeInTheDocument();
    expect(api.progressOverview).toHaveBeenCalledWith("days90");
    const o = f.progressOverview;
    expect(screen.getByText(`${o.wins}W`)).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "MMR" })).toBeInTheDocument();
    const movers = screen.getByRole("region", { name: "Packs that moved" });
    expect(within(movers).getByText(o.movers[0].pack.name)).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Time per week" })).toBeInTheDocument();
  });

  it("shows play time as hours and minutes, with the weekly average on its own line", async () => {
    renderView();
    const card = (await screen.findByText("Played")).closest(".d-kpi") as HTMLElement;
    expect(card.querySelector(".d-kpi__v")).toHaveTextContent(/^3h 55m$/);
    expect(within(card).getByText("18 min a week · 6 sessions")).toBeInTheDocument();
  });

  it("opens a ranked mode from its card", async () => {
    const { onSelect } = renderView();
    await userEvent.click(await screen.findByRole("button", { name: "Open ranked 2v2" }));
    expect(onSelect).toHaveBeenCalledWith({ kind: "mode", teamSize: 2 });
  });

  it("says when the range has no ranked matches or no training", async () => {
    renderView({ ...f.progressOverview, modes: [], mmr: [], ranked_matches: 0, wins: 0, losses: 0, attempts: 0, make_rate: null, movers: [], packs_played: 0 });
    expect(await screen.findByText("No ranked matches in this range")).toBeInTheDocument();
    expect(screen.getByText("No training in this range")).toBeInTheDocument();
  });
});
