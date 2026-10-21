import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Radar, band, takeaway, type RadarAxis } from "./Radar";

const core: RadarAxis[] = [
  { key: "boost", label: "Boost collected", t: 50, importance: 0.6, you: "2,310", winnersValue: "2,565", losersValue: "2,100", unit: "per 5 min" },
  { key: "zero", label: "Time at 0 boost", t: -20, importance: 0.8, you: "15%", winnersValue: "9%", losersValue: "14%", unit: "% of the time", lessBetter: true },
  { key: "behind", label: "Behind the ball", t: 120, importance: 0.4, you: "69%", winnersValue: "66%", losersValue: "58%", unit: "% of the time" },
  { key: "speed", label: "Average speed", t: 100, importance: 0.7, you: "66", winnersValue: "66", losersValue: "61", unit: "km/h" },
];

const plotOf = (name: string) => screen.getByRole("group", { name: `${name}. Arrow keys read each stat.` });
const cardOf = (c: HTMLElement) => c.querySelector(".c-radar__card") as HTMLElement;

describe("Radar", () => {
  it("labels each axis with its name, with a 3-step bar scaled by importance", () => {
    const { container } = render(<Radar label="Core" axes={core} />);
    expect([...container.querySelectorAll(".c-radar__label")].map((t) => t.textContent)).toEqual(core.map((a) => a.label));
    const steps = [...container.querySelectorAll(".c-radar__imp")].map((g) => g.querySelectorAll(".c-radar__imp-on").length);
    expect(steps).toEqual([2, 3, 1, 3]); // 0.6, 0.8 (most), 0.4 (least), 0.7 of 0.4–0.8
  });

  it("says what to work on, and how many are at the winners' level", () => {
    const { container } = render(<Radar label="Core" axes={core} />);
    expect(container.querySelector(".c-radar__take")?.textContent).toBe("Work on Time at 0 boost and Boost collected · 2 of 4 already at the winners' level");
  });

  it("always draws the losers' and winners' rings, with no automatic zoom", () => {
    const { container } = render(<Radar label="Core" axes={core} />);
    expect(container.querySelector(".c-radar__ring-lose")).not.toBeNull();
    expect(container.querySelector(".c-radar__ring-win")).not.toBeNull();
    expect(screen.queryByText(/zoomed/)).toBeNull();
  });

  it("shows a stat's card from the keyboard: You, Winners, Losers, verdict and weight; Escape closes it", () => {
    const { container } = render(<Radar label="Core" axes={core} />);
    const plot = plotOf("Core");
    fireEvent.keyDown(plot, { key: "ArrowRight" });
    const card = cardOf(container);
    expect(within(card).getByText("Boost collected")).toBeInTheDocument();
    expect(within(card).getByText("2,310")).toBeInTheDocument();
    expect(within(card).getByText("2,565")).toBeInTheDocument();
    expect(within(card).getByText("2,100")).toBeInTheDocument();
    expect(within(card).getByText("50% of the way to the winners")).toBeInTheDocument();
    expect(within(card).getByText("Matters for winning in your games")).toBeInTheDocument();
    fireEvent.keyDown(plot, { key: "ArrowLeft" });
    const last = cardOf(container);
    expect(within(last).getByText("Average speed")).toBeInTheDocument();
    expect(within(last).getByText("At the winners' level")).toBeInTheDocument();
    fireEvent.keyDown(plot, { key: "Escape" });
    expect(container.querySelector(".c-radar__card")).toBeNull();
  });

  it("says when less is better in the card", () => {
    const { container } = render(<Radar label="Core" axes={core} />);
    fireEvent.keyDown(plotOf("Core"), { key: "ArrowRight" });
    fireEvent.keyDown(plotOf("Core"), { key: "ArrowRight" });
    const card = cardOf(container);
    expect(within(card).getByText("% of the time · less is better")).toBeInTheDocument();
    expect(within(card).getByText("Below the losers' level")).toBeInTheDocument();
    expect(within(card).getByText("Matters a lot for winning in your games")).toBeInTheDocument();
  });

  it("reads the open stat to screen readers and lists every stat", () => {
    render(<Radar label="Core" axes={core} />);
    fireEvent.keyDown(plotOf("Core"), { key: "ArrowRight" });
    expect(screen.getByText("Boost collected: you 2,310, winners 2,565, losers 2,100. 50% of the way to the winners.", { selector: ".c-radar__live" })).toBeInTheDocument();
    expect(within(screen.getByRole("list")).getAllByRole("listitem")[2].textContent).toBe(
      "Behind the ball: you 69%, winners 66%, losers 58%. Past the winners' level, more isn't always better.",
    );
  });

  it("shows only the verdict for a category axis with no real values", () => {
    const { container } = render(<Radar label="Cats" axes={[{ key: "b", label: "Boost", t: 60, importance: 0.5, unit: "average of 3 stats" }, ...core.slice(1)]} />);
    fireEvent.keyDown(plotOf("Cats"), { key: "ArrowRight" });
    const card = cardOf(container);
    expect(within(card).getByText("60% of the way to the winners")).toBeInTheDocument();
    expect(card.querySelector(".c-radar__vals")).toBeNull();
  });

  it("dims an axis, explains it in the card, and reports clicks on labels", () => {
    const onAxis = vi.fn();
    const { container } = render(
      <Radar label="r" axes={[...core.slice(0, 2), { key: "m", label: "Mechanics", t: null, importance: 0, dim: true, tip: "Nothing in Mechanics separated winners from losers." }]} onAxis={onAxis} />,
    );
    fireEvent.click(screen.getByText("Time at 0 boost", { selector: "text" }));
    expect(onAxis).toHaveBeenCalledWith("zero");
    expect(screen.getByText("Mechanics")).toHaveClass("c-radar__dim");
    const plot = plotOf("r");
    ["ArrowRight", "ArrowRight", "ArrowRight"].forEach((key) => fireEvent.keyDown(plot, { key }));
    expect(within(cardOf(container)).getByText("Nothing in Mechanics separated winners from losers.")).toBeInTheDocument();
  });
});

describe("band", () => {
  it("names the zones", () => {
    expect([-1, 0, 39, 40, 84, 85, 115, 116].map((t) => band(t).tone)).toEqual(["red", "amber", "amber", "amber", "amber", "green", "green", "grey"]);
    expect(band(62.4).text).toBe("62% of the way to the winners");
  });
});

describe("takeaway", () => {
  it("praises when everything is at the winners' level, and ignores dimmed and unmeasured axes", () => {
    const t = takeaway([{ key: "a", label: "A", t: 90, importance: 1 }, { key: "b", label: "B", t: 10, importance: 1, dim: true }, { key: "c", label: "C", t: null, importance: 1 }]);
    expect(t).toEqual({ work: [], atWinners: 1, total: 1 });
    expect(takeaway([])).toBeNull();
  });
});
