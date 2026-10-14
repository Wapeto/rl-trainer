import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { DesktopApi } from "../api";
import { desktopFixtures as f, PROGRESS_NOW } from "../fixtures";
import { fakeDesktopApi, memoryStorage } from "../test/fakeApi";
import { ProgressPage } from "./ProgressPage";

function renderPage(api: DesktopApi = fakeDesktopApi(), storage = memoryStorage()) {
  const onOpenSession = vi.fn();
  const onOpenPack = vi.fn();
  render(
    <ProgressPage
      api={api}
      snapshot={null}
      now={PROGRESS_NOW}
      notify={vi.fn()}
      reload="r"
      onChanged={vi.fn()}
      storage={storage}
      onOpenSession={onOpenSession}
      onOpenPack={onOpenPack}
    />,
  );
  return { api, storage, onOpenSession, onOpenPack };
}

describe("ProgressPage", () => {
  it("lists the overview, ranked modes and packs of the last 90 days", async () => {
    const { api } = renderPage();
    const list = await screen.findByRole("navigation", { name: "Progress" });
    expect(api.progressList).toHaveBeenCalledWith("days90");
    expect(within(list).getByRole("radio", { name: "90 d" })).toHaveAttribute("aria-checked", "true");
    expect(within(list).getByRole("button", { name: /Overview/ })).toHaveAttribute("aria-current", "true");
    expect(within(list).getByRole("button", { name: /^2v2/ })).toBeInTheDocument();
    expect(within(list).getByRole("button", { name: /Uncomfortable Saves/ })).toBeInTheDocument();
  });

  it("remembers the range", async () => {
    const { api, storage } = renderPage();
    await userEvent.click(await screen.findByRole("radio", { name: "All" }));
    expect(api.progressList).toHaveBeenLastCalledWith("all");
    expect(storage.data["rl-progress-range"]).toBe("all");
  });

  it("starts from the remembered range", async () => {
    const { api } = renderPage(fakeDesktopApi(), memoryStorage({ "rl-progress-range": "days30" }));
    await screen.findByRole("navigation", { name: "Progress" });
    expect(api.progressList).toHaveBeenCalledWith("days30");
  });

  it("sorts packs by played, change or name", async () => {
    renderPage();
    const list = await screen.findByRole("navigation", { name: "Progress" });
    await userEvent.click(within(list).getByRole("button", { name: "Sort packs" }));
    await userEvent.click(within(list).getByRole("option", { name: "Name" }));
    const names = within(list)
      .getAllByRole("button")
      .map((b) => b.textContent ?? "")
      .filter((t) => t.includes("attempts"));
    expect(names[0]).toMatch(/^Uncomfortable Saves/);
    expect(names[names.length - 1]).toMatch(/^Wall Clears/);
  });

  it("shows the empty state and switches to All", async () => {
    const empty = { ...f.progressList, sessions: 0, played_ms: 0, modes: [], packs: [] };
    const { api } = renderPage(fakeDesktopApi({ progressList: vi.fn().mockResolvedValue(empty) }));
    expect(await screen.findByText("Nothing played in the last 90 days.")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Show all" }));
    expect(api.progressList).toHaveBeenLastCalledWith("all");
  });
});
