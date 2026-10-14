import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Nudge } from "@shared/generated/types";
import { NudgeBanner } from "./NudgeBanner";

const drop: Nudge = {
  kind: "rate_drop",
  fired_at_attempt: 412,
  last20_rate: 0.1,
  best20_rate: 0.4,
};
const streak: Nudge = { kind: "miss_streak", fired_at_attempt: 430, count: 14 };

function memory() {
  const data: Record<string, string> = {};
  return {
    getItem: (k: string) => data[k] ?? null,
    setItem: (k: string, v: string) => void (data[k] = v),
  };
}

describe("NudgeBanner", () => {
  it("shows the rate drop with its numbers and buzzes once", () => {
    const buzz = vi.fn();
    render(<NudgeBanner nudge={drop} buzz={buzz} storage={memory()} />);
    expect(screen.getByRole("status")).toHaveTextContent("Rate dropping");
    expect(screen.getByText("Last 20: 10% · your best today: 40%")).toBeInTheDocument();
    expect(screen.getByText("Take a 2-min break or switch pack.")).toBeInTheDocument();
    expect(buzz).toHaveBeenCalledTimes(1);
  });

  it("shows a miss streak with its growing count", () => {
    render(<NudgeBanner nudge={streak} buzz={vi.fn()} storage={memory()} />);
    expect(screen.getByText("14 misses in a row")).toBeInTheDocument();
    expect(screen.getByText("Reset your head: slow the first touch, or switch pack.")).toBeInTheDocument();
  });

  it("does not buzz again for the same nudge after a re-render or a reload", () => {
    const buzz = vi.fn();
    const storage = memory();
    const { rerender, unmount } = render(<NudgeBanner nudge={drop} buzz={buzz} storage={storage} />);
    rerender(<NudgeBanner nudge={{ ...drop, last20_rate: 0.05 }} buzz={buzz} storage={storage} />);
    unmount();
    render(<NudgeBanner nudge={drop} buzz={buzz} storage={storage} />);
    expect(buzz).toHaveBeenCalledTimes(1);
  });

  it("hides on a tap until the next nudge, and when the nudge ends", async () => {
    const storage = memory();
    const { rerender } = render(<NudgeBanner nudge={drop} buzz={vi.fn()} storage={storage} />);
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(screen.queryByRole("status")).toBeNull();
    rerender(<NudgeBanner nudge={streak} buzz={vi.fn()} storage={storage} />);
    expect(screen.getByRole("status")).toBeInTheDocument();
    rerender(<NudgeBanner nudge={null} buzz={vi.fn()} storage={storage} />);
    expect(screen.queryByRole("status")).toBeNull();
  });
});
