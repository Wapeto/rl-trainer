import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { desktopFixtures as f, PROGRESS_NOW } from "../fixtures";
import { fakeDesktopApi, memoryStorage } from "../test/fakeApi";
import { PROGRESS } from "@shared/glossary";
import { PackProgressView } from "./PackProgressView";

function renderView(api = fakeDesktopApi()) {
  const onOpenPack = vi.fn();
  const onOpenSession = vi.fn();
  render(
    <PackProgressView
      api={api}
      snapshot={null}
      now={PROGRESS_NOW}
      notify={vi.fn()}
      reload="r"
      onChanged={vi.fn()}
      storage={memoryStorage()}
      onOpenSession={onOpenSession}
      onOpenPack={onOpenPack}
      range="days90"
      packId={1}
    />,
  );
  return { api, onOpenPack, onOpenSession };
}

describe("PackProgressView", () => {
  it("explains the chart's lines, points and bars in a legend", async () => {
    renderView();
    const legend = await screen.findByRole("list", { name: "Chart legend" });
    const items = within(legend).getAllByRole("listitem").map((li) => li.textContent);
    expect(items).toEqual([
      `Target ${Math.round(f.progressPack.pack.target_rate * 100)}%`,
      "Average of the last 3 sessions",
      "Under 10 attempts",
      "Attempts per session",
    ]);
  });

  it("explains the tiles and the chart on hover", async () => {
    renderView();
    await userEvent.hover(await screen.findByText("Best session"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent(PROGRESS.best_session.text);
  });

  it("shows the cards, the make-rate chart with its target, bars and hollow points", async () => {
    const { api } = renderView();
    const p = f.progressPack;
    expect(await screen.findByRole("heading", { level: 1, name: p.pack.name })).toBeInTheDocument();
    expect(api.progressPack).toHaveBeenCalledWith(1, "days90");
    expect(screen.getByText(p.pack.code)).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Make rate" })).toBeInTheDocument();
    expect(screen.getByText(/drawn as hollow points/)).toBeInTheDocument();
  });

  it("opens the pack in Packs and a session from a point", async () => {
    const { onOpenPack, onOpenSession } = renderView();
    await userEvent.click(await screen.findByRole("button", { name: "Open in Packs ›" }));
    expect(onOpenPack).toHaveBeenCalledWith(1);
    const last = f.progressPack.points[f.progressPack.points.length - 1];
    await userEvent.click(screen.getAllByLabelText(new RegExp(`${last.makes} of ${last.attempts}`))[0]);
    expect(onOpenSession).toHaveBeenCalledWith(last.session_id);
  });

  it("says when the pack was not played in the range", async () => {
    renderView(fakeDesktopApi({ progressPack: vi.fn().mockResolvedValue(null) }));
    expect(await screen.findByText("This pack no longer exists.")).toBeInTheDocument();
  });
});
