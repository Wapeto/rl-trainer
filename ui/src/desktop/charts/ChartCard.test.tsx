import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ChartCard } from "./ChartCard";
import { WeekBars } from "./WeekBars";

const chart = {
  series: [
    {
      label: "Goals",
      color: "#7aa7ff",
      points: [
        { at: 1, value: 0.5, sessionId: 1, tip: ["0.5", "1 Oct"] },
        { at: 2, value: 1.2, sessionId: 2, tip: ["1.2", "8 Oct"] },
      ],
    },
  ],
};

const GLOSS = { title: "Goals scored", text: "Goals you scored, per match." };

describe("ChartCard", () => {
  it("explains a plain card on its title", async () => {
    render(<ChartCard title="Goals" value="0.9 / game" chart={chart} gloss={GLOSS} />);
    await userEvent.hover(screen.getByText("Goals"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Goals you scored, per match.");
  });

  it("explains a card that can be chosen, and gets out of the way when it is chosen", async () => {
    const onSelect = vi.fn();
    render(<ChartCard title="Goals" value="0.9 / game" chart={chart} gloss={GLOSS} selected={false} onSelect={onSelect} />);
    const card = screen.getByRole("button", { name: "Goals" });
    await userEvent.hover(card);
    expect(await screen.findByRole("tooltip")).toHaveTextContent("Goals you scored, per match.");
    await userEvent.click(card);
    expect(onSelect).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("is a toggle that chooses the stat when it can be chosen", async () => {
    const onSelect = vi.fn();
    render(<ChartCard title="Goals" value="0.9 / game" chart={chart} selected={false} onSelect={onSelect} />);
    const card = screen.getByRole("button", { name: "Goals" });
    expect(card).toHaveAttribute("aria-pressed", "false");
    await userEvent.click(card);
    expect(onSelect).toHaveBeenCalledTimes(1);
  });

  it("has nothing to open when it cannot be chosen", () => {
    render(<ChartCard title="Goals" value="0.9 / game" chart={chart} />);
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("opens the session, not the choice, when a point is clicked", async () => {
    const onOpenSession = vi.fn();
    const onSelect = vi.fn();
    render(<ChartCard title="Goals" value="0.9" chart={{ ...chart, onOpenSession }} onSelect={onSelect} />);
    await userEvent.click(screen.getByLabelText("1.2 · 8 Oct"));
    expect(onOpenSession).toHaveBeenCalledWith(2);
    expect(onSelect).not.toHaveBeenCalled();
  });

  it("shows a note under the title", () => {
    render(<ChartCard title="Big pads" value="8.4 / game" note="estimated" chart={chart} />);
    expect(screen.getByText("estimated")).toBeInTheDocument();
  });
});

const WEEKS = [
  { start: new Date(2026, 8, 28).getTime(), training: 3_600_000, freeplay: 0, ranked: 1_800_000, other: 0 },
  { start: new Date(2026, 9, 5).getTime(), training: 0, freeplay: 0, ranked: 0, other: 900_000 },
];

describe("WeekBars", () => {
  it("draws one column per week, with a title per week", () => {
    const { container } = render(
      <WeekBars
        weeks={[
          { start: new Date(2026, 8, 28).getTime(), training: 3_600_000, freeplay: 0, ranked: 1_800_000, other: 0 },
          { start: new Date(2026, 9, 5).getTime(), training: 0, freeplay: 0, ranked: 0, other: 0 },
        ]}
      />,
    );
    expect(container.querySelectorAll(".c-week")).toHaveLength(2);
    expect(screen.getByText("Week of 28 Sept · 1h 30")).toBeInTheDocument();
  });

  it("names each colour with its time over the shown weeks and dates the first and last week", () => {
    render(<WeekBars weeks={WEEKS} />);
    const legend = screen.getByRole("list", { name: "Kinds of play" });
    const items = within(legend).getAllByRole("listitem").map((li) => li.textContent);
    expect(items).toEqual(["Training1h 00", "Free play0 min", "Ranked30 min", "Casual & other15 min"]);
    expect(screen.getByText("28 Sept")).toBeInTheDocument();
    expect(screen.getByText("5 Oct")).toBeInTheDocument();
  });

  it("explains a week on hover in the chart's own tooltip", async () => {
    const { container } = render(<WeekBars weeks={WEEKS} />);
    expect(container.querySelector("[title]")).toBeNull();
    await userEvent.hover(container.querySelectorAll(".c-week")[0]);
    const tip = screen.getByRole("tooltip");
    expect(tip).toHaveTextContent("Week of 28 Sept");
    expect(tip).toHaveTextContent("Training 1h 00");
    expect(tip).toHaveTextContent("Ranked 30 min");
    expect(tip).not.toHaveTextContent("Free play");
  });
});
