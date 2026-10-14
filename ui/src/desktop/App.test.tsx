import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { fixtures } from "@shared/fixtures";
import { DesktopApp } from "./App";
import { DESKTOP_NOW, desktopFixtures } from "./fixtures";
import { fakeDesktopApi, feedOf, memoryStorage } from "./test/fakeApi";

function renderApp(storage = memoryStorage(), snapshot = fixtures.trainingBelow) {
  const api = fakeDesktopApi();
  render(<DesktopApp api={api} feed={feedOf(snapshot)} now={DESKTOP_NOW} storage={storage} />);
  return { api, storage };
}

describe("DesktopApp", () => {
  it("opens on Sessions with the review badge and the status lights", async () => {
    renderApp();
    expect(screen.getByRole("button", { name: /Sessions/ })).toHaveAttribute("aria-current", "page");
    expect(await screen.findByLabelText("1 to review")).toBeInTheDocument();
    expect(screen.getByText("Rocket League running")).toBeInTheDocument();
    expect(screen.getByText("Phone connected")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: /Progress/ }));
    expect(await screen.findByRole("navigation", { name: "Progress" })).toBeInTheDocument();
  });

  it("switches pages", async () => {
    renderApp();
    await userEvent.click(screen.getByRole("button", { name: /Packs/ }));
    expect(screen.getByRole("button", { name: /Packs/ })).toHaveAttribute("aria-current", "page");
    await userEvent.click(screen.getByRole("button", { name: /Settings/ }));
    expect(screen.getByRole("button", { name: /Settings/ })).toHaveAttribute("aria-current", "page");
  });

  it("folds the sidebar into a rail and remembers it", async () => {
    const { storage } = renderApp();
    await userEvent.click(screen.getByRole("button", { name: "Collapse the sidebar" }));
    expect(screen.getByRole("complementary", { name: "Main" })).toHaveClass("d-side--rail");
    expect(storage.data["rl-desktop-sidebar-folded"]).toBe("1");
    expect(screen.getByRole("button", { name: "Expand the sidebar" })).toBeInTheDocument();
  });

  it("starts folded when it was folded last time", () => {
    renderApp(memoryStorage({ "rl-desktop-sidebar-folded": "1" }), fixtures.idle);
    expect(screen.getByRole("complementary", { name: "Main" })).toHaveClass("d-side--rail");
    expect(screen.getByText("Rocket League closed")).toBeInTheDocument();
  });

  it("deletes the newest session without saying it vanished", async () => {
    let deleted = false;
    const api = fakeDesktopApi({
      // The list query is slower than the one-session query, as in the app.
      sessions: vi.fn(async () => {
        if (!deleted) return desktopFixtures.cards;
        await new Promise((resolve) => setTimeout(resolve, 30));
        return desktopFixtures.cards.slice(1);
      }),
      session: vi.fn(async (id: number) =>
        id === 12 ? (deleted ? null : desktopFixtures.detail) : { ...desktopFixtures.detail, id, previous_id: null },
      ),
      deleteSession: vi.fn(async () => {
        deleted = true;
      }),
    });
    render(<DesktopApp api={api} feed={feedOf(fixtures.idle)} now={DESKTOP_NOW} storage={memoryStorage()} />);
    await userEvent.click(await screen.findByRole("button", { name: "Session actions" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Delete session…" }));
    const dialog = screen.getByRole("dialog", { name: "Delete this session?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(api.session).toHaveBeenLastCalledWith(11));
    await screen.findByRole("heading", { level: 1 });
    expect(screen.queryByText("This session no longer exists.")).toBeNull();
  });

  it("counts ranked replays to save in a blue badge on Sessions", async () => {
    renderApp();
    expect(await screen.findByLabelText("1 replay to save")).toHaveClass("d-badge--blue");
  });
});
