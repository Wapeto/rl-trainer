import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import { FIXTURE_NOW, fixtures } from "@shared/fixtures";
import { fakeApi, fakeKeepAwake, mutableSource, staticSource } from "../test/helpers";
import { PhoneApp } from "./App";

function renderApp(state: Parameters<typeof staticSource>[0]) {
  return render(<PhoneApp source={staticSource(state)} api={fakeApi()} now={FIXTURE_NOW} />);
}

describe("PhoneApp", () => {
  it("asks to scan the QR code when not paired", () => {
    renderApp({ conn: "unpaired", snapshot: null });
    expect(screen.getByRole("heading", { name: "Scan the QR code on the PC" })).toBeInTheDocument();
  });

  it("shows the last session while idle", () => {
    renderApp({ conn: "live", snapshot: fixtures.idle });
    expect(screen.getByRole("heading", { name: "Last session" })).toBeInTheDocument();
    expect(screen.getByText("Rocket League closed")).toBeInTheDocument();
    expect(screen.getByText("W3 · L1")).toBeInTheDocument();
    expect(screen.getByText("“Nice warm-up 1v1 match”")).toBeInTheDocument();
    expect(screen.getByText("632")).toBeInTheDocument();
  });

  it("shows this session so far while in the menus", () => {
    render(<PhoneApp source={staticSource({ conn: "live", snapshot: fixtures.menus })} api={fakeApi()} now={FIXTURE_NOW - 10_000} />);
    expect(screen.getByRole("heading", { name: "This session" })).toBeInTheDocument();
    expect(screen.getByText("W1 · L0")).toBeInTheDocument();
    expect(screen.getByText("Training made")).toBeInTheDocument();
    expect(screen.getByText("58%")).toBeInTheDocument();
  });

  it("asks for MMR in the menus and saves it without ending the session", async () => {
    const api = fakeApi();
    const { container } = render(
      <PhoneApp source={staticSource({ conn: "live", snapshot: fixtures.menus })} api={api} now={FIXTURE_NOW} />,
    );
    expect(container.querySelector("main")).toHaveAttribute("data-screen", "session_end");
    expect(screen.getByText("In the menus")).toBeInTheDocument();
    await userEvent.click(await screen.findByRole("button", { name: "Next: MMR & note" }));
    expect(screen.getByRole("spinbutton", { name: "MMR 2v2" })).toHaveAttribute("aria-valuenow", "899");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.reviewSession).toHaveBeenCalledWith(12, [{ team_size: 2, mmr: 899 }], "");
  });

  it("keeps the screen on from the first tap anywhere", async () => {
    const awake = fakeKeepAwake();
    render(
      <PhoneApp source={staticSource({ conn: "live", snapshot: fixtures.idle })} api={fakeApi()} now={FIXTURE_NOW} keepAwake={awake} />,
    );
    expect(screen.getByText("Tap to keep the screen on")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("heading", { name: "Last session" }));
    expect(awake.enable).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("Tap to keep the screen on")).toBeNull();
  });

  it("does not offer it before pairing", () => {
    render(<PhoneApp source={staticSource({ conn: "unpaired", snapshot: null })} api={null} keepAwake={fakeKeepAwake()} />);
    expect(screen.queryByText("Tap to keep the screen on")).toBeNull();
  });

  it("explains an empty history", () => {
    renderApp({ conn: "live", snapshot: fixtures.idleEmpty });
    expect(screen.getByRole("heading", { name: "No sessions yet" })).toBeInTheDocument();
  });

  it("dims the screen and says so while reconnecting", () => {
    const { container } = renderApp({ conn: "reconnecting", snapshot: fixtures.idle });
    expect(screen.getByText("Reconnecting…")).toBeInTheDocument();
    expect(container.querySelector("main")).toHaveClass("phone--dim");
  });

  it("follows the session closing without a reload", () => {
    const source = mutableSource({ conn: "live", snapshot: fixtures.trainingBelow });
    const { container } = render(<PhoneApp source={source} api={fakeApi()} now={FIXTURE_NOW} />);
    expect(container.querySelector("main")).toHaveAttribute("data-screen", "training");
    act(() => source.set({ conn: "live", snapshot: fixtures.sessionEnd }));
    expect(container.querySelector("main")).toHaveAttribute("data-screen", "session_end");
  });

  it("starts a fresh review form when another session is waiting for review", async () => {
    const source = mutableSource({ conn: "live", snapshot: fixtures.sessionEnd });
    render(<PhoneApp source={source} api={fakeApi()} now={FIXTURE_NOW} />);
    await userEvent.click(await screen.findByRole("button", { name: "Next: MMR & note" }));
    await userEvent.type(screen.getByLabelText("Note"), " and more");
    const next = structuredClone(fixtures.sessionEnd);
    const last = next.live.last_session!;
    last.id = 12;
    last.notes = "";
    last.ranked_team_sizes = [2];
    act(() => source.set({ conn: "live", snapshot: next }));
    await userEvent.click(await screen.findByRole("button", { name: "Next: MMR & note" }));
    expect(screen.getByLabelText("Note")).toHaveValue("");
    expect(screen.queryByRole("spinbutton", { name: "MMR 1v1" })).toBeNull();
    expect(screen.getByRole("spinbutton", { name: "MMR 2v2" })).toHaveAttribute("aria-valuenow", "899");
  });
});
