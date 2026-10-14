import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { FIXTURE_NOW, fixtures } from "@shared/fixtures";
import type { SessionSummary } from "@shared/generated/types";
import { desktopFixtures } from "../../desktop/fixtures";
import { fakeApi } from "../../test/helpers";
import { SessionEnd, isGoodSession } from "./SessionEnd";

function renderEnd(api = fakeApi(), notify = vi.fn()) {
  const utils = render(
    <SessionEnd snap={fixtures.sessionEnd} conn="live" now={FIXTURE_NOW} api={api} notify={notify} />,
  );
  return { ...utils, api, notify };
}

describe("SessionEnd", () => {
  it("shows the review first, then the MMR step", async () => {
    renderEnd();
    expect(await screen.findByRole("heading", { name: "Session review" })).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Next: MMR & note" }));
    expect(screen.getByRole("spinbutton", { name: "MMR 1v1" })).toHaveAttribute("aria-valuenow", "632");
    expect(screen.getByRole("spinbutton", { name: "MMR 2v2" })).toHaveAttribute("aria-valuenow", "899");
    await userEvent.click(screen.getByRole("button", { name: "Back to review" }));
    expect(screen.getByRole("heading", { name: "Session review" })).toBeInTheDocument();
  });

  it("shows the coach recap in one card with its summary line", async () => {
    renderEnd();
    const card = await screen.findByRole("region", { name: "Coach review" });
    expect(card).toHaveClass("sr-card");
    expect(within(card).getByText("2 things stood out · 1 replay to save")).toBeInTheDocument();
    expect(within(card).getByRole("group", { name: /^Touches under 20 boost 49%/ })).toBeInTheDocument();
  });

  it("skips the review step when the review is empty", async () => {
    const empty = {
      ...desktopFixtures.review,
      training: [],
      modes: [],
      to_save: [],
    };
    renderEnd(fakeApi({ sessionReview: vi.fn().mockResolvedValue(empty) }));
    expect(await screen.findByRole("spinbutton", { name: "MMR 1v1" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Back to review" })).toBeNull();
  });

  it("goes straight to the MMR step when the review cannot load", async () => {
    renderEnd(
      fakeApi({
        sessionReview: vi.fn().mockRejectedValue(new Error("offline")),
      }),
    );
    expect(await screen.findByRole("spinbutton", { name: "MMR 1v1" })).toBeInTheDocument();
  });

  it("saves the wheel values and the note", async () => {
    const { api } = renderEnd();
    await userEvent.click(await screen.findByRole("button", { name: "Next: MMR & note" }));
    const wheel = screen.getByRole("spinbutton", { name: "MMR 2v2" });
    wheel.focus();
    await userEvent.keyboard("{PageUp}{ArrowUp}{ArrowUp}{ArrowUp}");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.reviewSession).toHaveBeenCalledWith(
      11,
      [
        { team_size: 1, mmr: 632 },
        { team_size: 2, mmr: 912 },
      ],
      "Nice warm-up 1v1 match",
    );
  });

  it("can be skipped", async () => {
    const { api } = renderEnd();
    await userEvent.click(await screen.findByRole("button", { name: "Next: MMR & note" }));
    await userEvent.click(screen.getByRole("button", { name: "Skip" }));
    expect(api.reviewSession).toHaveBeenCalledWith(11, [], null);
  });

  it("summarises the session on the MMR step with the last value of each mode", async () => {
    const { container } = renderEnd();
    await userEvent.click(await screen.findByRole("button", { name: "Next: MMR & note" }));
    expect(container.firstChild).toHaveClass("tone-green");
    expect(screen.getByRole("heading", { name: "Good session" })).toBeInTheDocument();
    expect(screen.getByText("last 632")).toBeInTheDocument();
    expect(screen.queryByRole("spinbutton", { name: "MMR 3v3" })).toBeNull();
  });

  it("calls a session good on more wins or on reaching the targets", () => {
    const s = fixtures.sessionEnd.live.last_session!;
    expect(
      isGoodSession({
        ...s,
        wins: 1,
        losses: 2,
        training_make_rate: 0.5,
        training_target_rate: 0.65,
      }),
    ).toBe(false);
    expect(
      isGoodSession({
        ...s,
        wins: 0,
        losses: 0,
        training_make_rate: 0.7,
        training_target_rate: 0.65,
      }),
    ).toBe(true);
    expect(
      isGoodSession({
        ...s,
        wins: 2,
        losses: 1,
        training_make_rate: null,
        training_target_rate: null,
      }),
    ).toBe(true);
  });

  it("sends the review once even when Save is pressed twice at once", async () => {
    const api = fakeApi({
      reviewSession: vi.fn(() => new Promise<SessionSummary>(() => {})),
    });
    renderEnd(api);
    await userEvent.click(await screen.findByRole("button", { name: "Next: MMR & note" }));
    const save = screen.getByRole("button", { name: "Save" });
    await act(async () => {
      save.click();
      save.click();
    });
    expect(api.reviewSession).toHaveBeenCalledTimes(1);
  });
});
