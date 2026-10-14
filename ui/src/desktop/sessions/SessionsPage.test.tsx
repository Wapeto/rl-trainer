import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SessionCard } from "@shared/generated/types";
import type { DesktopApi } from "../api";
import { DESKTOP_NOW, desktopFixtures as f } from "../fixtures";
import { longDay } from "../format";
import type { PageProps } from "../pageProps";
import { fakeDesktopApi } from "../test/fakeApi";
import { PAGE_SIZE, SessionsPage } from "./SessionsPage";

function renderPage(api: DesktopApi = fakeDesktopApi(), extra: Partial<PageProps> = {}) {
  const notify = vi.fn();
  const onChanged = vi.fn();
  render(
    <SessionsPage api={api} snapshot={null} now={DESKTOP_NOW} notify={notify} reload="r" onChanged={onChanged} {...extra} />,
  );
  return { api, notify, onChanged };
}

describe("SessionsPage", () => {
  it("lists sessions with their summary and review tag, and opens the newest", async () => {
    renderPage();
    const list = screen.getByRole("region", { name: "Sessions" });
    expect(await within(list).findByText("2 packs · 3 ranked")).toBeInTheDocument();
    expect(within(list).getByText("1 pack · 2 ranked")).toBeInTheDocument();
    expect(within(list).getByText("Free play")).toBeInTheDocument();
    expect(within(list).getAllByText("Review")).toHaveLength(1);
    expect(await screen.findByRole("heading", { level: 1, name: longDay(f.detail.started_at) })).toBeInTheDocument();
    expect(screen.getByText("912")).toBeInTheDocument();
    expect(screen.getByText("+13")).toBeInTheDocument();
    expect(screen.getByText(f.detail.notes)).toBeInTheDocument();
    expect(screen.getByText("146")).toBeInTheDocument();
    expect(screen.getByText("58%")).toBeInTheDocument();
    // The timeline's training block and the coach review's training line.
    await waitFor(() => expect(screen.getAllByText("Aerial Shots - Pass")).toHaveLength(2));
    expect(screen.getByText("No pack set")).toBeInTheDocument();
    // The timeline's match group (the coach review card says "Ranked 2v2 vs your usual" too).
    expect(within(await screen.findByRole("region", { name: "Coach review" })).getByText(/Ranked 2v2/)).toBeInTheDocument();
    expect(screen.getAllByText("Ranked 2v2", { exact: false })).toHaveLength(2);
  });

  it("opens another session", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByText("1 pack · 2 ranked"));
    expect(api.session).toHaveBeenLastCalledWith(11);
  });

  it("says when a session no longer exists and moves to the newest one", async () => {
    const api = fakeDesktopApi({
      sessions: vi.fn().mockResolvedValueOnce(f.cards).mockResolvedValue(f.cards.slice(1)),
      session: vi.fn(async (id: number) => (id === 12 ? null : { ...f.detail, id })),
    });
    renderPage(api);
    expect(await screen.findByText("This session no longer exists.")).toBeInTheDocument();
    await waitFor(() => expect(api.session).toHaveBeenLastCalledWith(11));
    expect(await screen.findByRole("heading", { level: 1 })).toBeInTheDocument();
  });

  it("pages with Show more, keeps the pages on a reload, and starts over on a search", async () => {
    const page = Array.from({ length: PAGE_SIZE }, (_, i): SessionCard => ({ ...f.cards[0], id: 100 + i }));
    const twoPages = [...page, ...page.map((c) => ({ ...c, id: c.id + PAGE_SIZE }))];
    const sessions = vi.fn((_search: string, _offset: number, limit: number) =>
      Promise.resolve(limit === PAGE_SIZE ? page : twoPages),
    );
    const api = fakeDesktopApi({ sessions });
    const props = { api, snapshot: null, now: DESKTOP_NOW, notify: vi.fn(), onChanged: vi.fn() };
    const { rerender } = render(<SessionsPage {...props} reload="r1" />);
    await userEvent.click(await screen.findByRole("button", { name: "Show more" }));
    expect(sessions).toHaveBeenLastCalledWith("", 0, 2 * PAGE_SIZE, false);
    rerender(<SessionsPage {...props} reload="r2" />);
    await waitFor(() => expect(sessions).toHaveBeenCalledTimes(3));
    expect(sessions).toHaveBeenLastCalledWith("", 0, 2 * PAGE_SIZE, false);
    await userEvent.type(screen.getByLabelText("Search sessions"), "wall");
    await waitFor(() => expect(sessions).toHaveBeenLastCalledWith("wall", 0, PAGE_SIZE, false));
  });

  it("sets the pack of a training activity, suggesting the one played before", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Set pack" }));
    const dialog = await screen.findByRole("dialog", { name: "Which pack was this?" });
    expect(await within(dialog).findByText("Played before this")).toBeInTheDocument();
    await userEvent.click(within(dialog).getByRole("button", { name: "Set" }));
    expect(api.setBlockPack).toHaveBeenCalledWith(12, 11);
  });

  it("splits at a gap after asking", async () => {
    const { api } = renderPage();
    // Only the 7-minute break before the matches; the 1-minute ones offer no split.
    const splits = await screen.findAllByRole("button", { name: "Split here" });
    expect(splits).toHaveLength(1);
    await userEvent.click(splits[0]);
    const dialog = screen.getByRole("dialog", { name: "Split this session here?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Split" }));
    expect(api.splitSession).toHaveBeenCalledWith(12, 12);
  });

  it("merges with the previous session and selects the result", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Session actions" }));
    expect(screen.getByRole("menuitem", { name: "Merge with next session" })).toBeDisabled();
    await userEvent.click(screen.getByRole("menuitem", { name: "Merge with previous session" }));
    const dialog = await screen.findByRole("dialog", { name: "Merge these two sessions?" });
    expect(dialog).toHaveTextContent("Both notes are kept");
    await userEvent.click(within(dialog).getByRole("button", { name: "Merge" }));
    expect(api.mergeSessions).toHaveBeenCalledWith(12, "previous");
    await waitFor(() => expect(api.session).toHaveBeenLastCalledWith(11));
  });

  it("deletes the session after saying what goes", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Session actions" }));
    await userEvent.click(screen.getByRole("menuitem", { name: "Delete session…" }));
    const dialog = screen.getByRole("dialog", { name: "Delete this session?" });
    expect(dialog).toHaveTextContent("Its 6 activities, 146 attempts, 3 matches and the MMR you entered are removed.");
    await userEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect(api.deleteSession).toHaveBeenCalledWith(12);
  });

  it("deletes one match from an opened group", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Show matches" }));
    expect(screen.getAllByRole("row")).toHaveLength(4);
    await userEvent.click(screen.getByRole("button", { name: "Delete the match on Mannfield" }));
    const dialog = screen.getByRole("dialog", { name: "Delete this activity?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Delete" }));
    expect(api.deleteActivity).toHaveBeenCalledWith(14);
  });

  it("keeps a session that is still being recorded read-only", async () => {
    const api = fakeDesktopApi({ session: vi.fn().mockResolvedValue(f.openDetail) });
    renderPage(api);
    expect((await screen.findAllByText("NOW")).length).toBeGreaterThan(0);
    expect(screen.queryByRole("button", { name: "Split here" })).toBeNull();
    expect(screen.getByRole("button", { name: "Edit review" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Session actions" }));
    expect(screen.getByRole("menuitem", { name: "Delete session…" })).toBeDisabled();
  });

  it("filters the list to sessions with replays to save", async () => {
    const api = fakeDesktopApi();
    renderPage(api);
    const lane = await screen.findByRole("button", { name: /1 replay to save/ });
    expect(lane).toHaveTextContent("oldest #2 of 20");
    await userEvent.hover(lane);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(/still in your in-game match history/);
    await userEvent.click(lane);
    expect(lane).toHaveAttribute("aria-pressed", "true");
    expect(api.sessions).toHaveBeenLastCalledWith("", 0, PAGE_SIZE, true);
    await userEvent.click(lane);
    expect(api.sessions).toHaveBeenLastCalledWith("", 0, PAGE_SIZE, false);
  });

  it("explains the session totals on hover", async () => {
    renderPage();
    await screen.findByRole("heading", { level: 1, name: longDay(f.detail.started_at) });
    const kpis = document.querySelector(".d-kpis") as HTMLElement;
    await userEvent.hover(within(kpis).getByText("Free play"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Minutes spent in free play this session.");
  });

  it("shows no lane when nothing is to save", async () => {
    renderPage(fakeDesktopApi({ replaysToSave: vi.fn().mockResolvedValue([]) }));
    await screen.findAllByRole("button", { name: /:/ });
    expect(screen.queryByRole("button", { name: /to save/ })).toBeNull();
  });

  it("loads more pages until a session opened from elsewhere is in the list", async () => {
    const many = Array.from({ length: 120 }, (_, i) => ({
      ...f.cards[0],
      id: 1000 - i,
      started_at: f.cards[0].started_at - i * 3_600_000,
    }));
    const api = fakeDesktopApi({
      sessions: vi.fn((_s: string, offset: number, limit: number) => Promise.resolve(many.slice(offset, offset + limit))),
    });
    renderPage(api, { focusId: 1000 - 110 });
    await waitFor(() => expect(api.sessions).toHaveBeenLastCalledWith("", 0, 3 * PAGE_SIZE, false));
    await waitFor(() => expect(document.querySelector(".d-lane--on")).not.toBeNull());
    // Each page waits for the one before it: it never races past the page that holds the session.
    expect(api.sessions).toHaveBeenCalledTimes(3);
  });

  it("opens a match's replay page and comes back to the session", async () => {
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Show matches" }));
    await userEvent.click(screen.getByRole("button", { name: "Replay" }));
    expect(await screen.findByRole("table", { name: "Key stats" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Edit review" })).toBeNull(); // hidden, so not in the a11y tree
    await userEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(screen.queryByRole("table", { name: "Key stats" })).toBeNull();
    expect(screen.getByRole("button", { name: "Hide matches" })).toBeVisible();
  });
});
