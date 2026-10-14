import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { DesktopApi } from "../api";
import { DESKTOP_NOW } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { PROGRESS } from "@shared/glossary";
import { PacksPage } from "./PacksPage";

function renderPage(api: DesktopApi = fakeDesktopApi()) {
  render(<PacksPage api={api} snapshot={null} now={DESKTOP_NOW} notify={vi.fn()} reload="r" onChanged={vi.fn()} />);
  return { api };
}

describe("PacksPage", () => {
  it("explains the pack's tiles on hover", async () => {
    renderPage();
    const main = screen.getByRole("main");
    await within(main).findByRole("heading", { level: 1, name: "Aerial Shots - Pass" });
    const label = within(main).getByText("Session by session");
    await userEvent.hover(label);
    expect(await screen.findByRole("tooltip")).toHaveTextContent(PROGRESS.session_by_session.text);
    await userEvent.unhover(label);
    await userEvent.hover(within(main).getByText("Overall"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent(PROGRESS.pack_overall.text);
  });

  it("lists visible packs with their rate and keeps hidden ones folded", async () => {
    renderPage();
    const list = screen.getByRole("region", { name: "Packs" });
    expect(await within(list).findByText("Aerial Shots - Pass")).toBeInTheDocument();
    expect(within(list).getByText("Never played")).toBeInTheDocument();
    expect(within(list).queryByText("Consistency")).toBeNull();
    await userEvent.click(within(list).getByRole("button", { name: /Hidden · 1/ }));
    expect(within(list).getByText("Consistency")).toBeInTheDocument();
  });

  it("shows the selected pack session by session", async () => {
    renderPage();
    const main = screen.getByRole("main");
    expect(await within(main).findByRole("heading", { level: 1, name: "Aerial Shots - Pass" })).toBeInTheDocument();
    expect(within(main).getByText("C7E0-9E0B-B739-A899")).toBeInTheDocument();
    expect(within(main).getByText("target 65%")).toBeInTheDocument();
    expect(within(main).getAllByText("60%")).toHaveLength(2); // overall, and one session row
    expect(within(main).getAllByText("70%")).toHaveLength(2); // best session, and its row
    expect(within(main).getAllByRole("img", { name: /against a 65% target/ })).toHaveLength(5);
    expect(within(main).getByText("Focus on the first touch.")).toBeInTheDocument();
  });

  it("sorts and searches through the app", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Sort packs" }));
    await userEvent.click(screen.getByRole("option", { name: "Most played" }));
    await waitFor(() => expect(api.packs).toHaveBeenLastCalledWith("most_played", ""));
    await userEvent.type(screen.getByLabelText("Search packs"), "c7e0");
    await waitFor(() => expect(api.packs).toHaveBeenLastCalledWith("most_played", "c7e0"));
  });

  it("hides the selected pack", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Hide" }));
    expect(api.setPackHidden).toHaveBeenCalledWith(11, true);
  });

  it("adds a pack and opens it", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Add" }));
    const dialog = screen.getByRole("dialog", { name: "Add a pack" });
    await userEvent.type(within(dialog).getByLabelText("Code"), "1111222233334444");
    await userEvent.type(within(dialog).getByLabelText("Name"), "Fresh");
    await userEvent.click(within(dialog).getByRole("button", { name: "Add pack" }));
    expect(api.addPack).toHaveBeenCalled();
    await waitFor(() => expect(api.pack).toHaveBeenLastCalledWith(42));
  });
});
