import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { MatchSummary } from "@shared/generated/types";
import { desktopFixtures as f } from "../fixtures";
import { ReplayCell } from "./ReplayCell";

const DAY = 86_400_000;
const withReplay = f.detail.activities.find((a) => a.match_summary?.replay)!.match_summary!;
const noReplay: MatchSummary = { ...withReplay, replay: null, history_place: 18 };

describe("ReplayCell", () => {
  it("opens the match page of a match that has a replay", async () => {
    const onOpen = vi.fn();
    render(<ReplayCell match={withReplay} startedAt={0} now={DAY} onOpen={onOpen} />);
    await userEvent.click(screen.getByRole("button", { name: "Replay" }));
    expect(onOpen).toHaveBeenCalledTimes(1);
  });

  it("asks to save a match still in history, amber and explained when it is about to go", async () => {
    render(<ReplayCell match={noReplay} startedAt={0} now={DAY} onOpen={vi.fn()} />);
    const chip = screen.getByText("Save soon");
    expect(chip).toHaveClass("d-chip--soon");
    await userEvent.tab();
    expect(screen.getByRole("tooltip")).toHaveTextContent("About place 18 of 20");
  });

  it("says Save before place 17", () => {
    render(<ReplayCell match={{ ...noReplay, history_place: 12 }} startedAt={0} now={DAY} onOpen={vi.fn()} />);
    expect(screen.getByText("Save")).not.toHaveClass("d-chip--soon");
  });

  it("says nothing once the match has left history", () => {
    const { container } = render(
      <ReplayCell match={{ ...noReplay, history_place: 21 }} startedAt={0} now={DAY} onOpen={vi.fn()} />,
    );
    expect(container).toBeEmptyDOMElement();
  });
});
