import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Radar, takeaway, type RadarAxis } from "./Radar";

const core: RadarAxis[] = [
  { key: "score", label: "Score", me: 52, winners: 66, you: "418", winnersValue: "471", unit: "per game" },
  { key: "goals", label: "Goals", me: 61, winners: 68, you: "0.9", winnersValue: "1.1", unit: "per 5 min" },
  { key: "saves", label: "Saves", me: 72, winners: 58, you: "1.6", winnersValue: "1.2", unit: "per 5 min" },
  { key: "shots", label: "Shots", me: 38, winners: 71, you: "2.1", winnersValue: "3.4", unit: "per 5 min" },
];

const plotOf = (name: string) => screen.getByRole("group", { name: `${name}. Arrow keys read each stat.` });

describe("Radar", () => {
  it("labels each axis with its name only, and says the takeaway", () => {
    const { container } = render(<Radar label="Core" axes={core} />);
    expect([...container.querySelectorAll(".c-radar__label")].map((t) => t.textContent)).toEqual(["Score", "Goals", "Saves", "Shots"]);
    expect(screen.getByText("Biggest gap: Shots · Ahead on Saves")).toBeInTheDocument();
  });

  it("zooms in automatically when the values sit in a narrow band, and says so", () => {
    render(<Radar label="Core" axes={core} />);
    expect(screen.getByText("Rank scale 30–80, zoomed in so the gaps are visible")).toBeInTheDocument();
  });

  it("keeps the full scale when the values spread", () => {
    render(<Radar label="Boost" axes={[...core.slice(0, 3), { key: "zero", label: "Time at 0", me: 9, winners: 94 }]} />);
    expect(screen.getByText("Rank scale 0–100")).toBeInTheDocument();
  });

  it("shows a stat's card from the keyboard, with both real values, and closes it with Escape", () => {
    const { container } = render(<Radar label="Core" axes={core} />);
    const plot = plotOf("Core");
    fireEvent.keyDown(plot, { key: "ArrowRight" });
    const card = container.querySelector(".c-radar__card") as HTMLElement;
    expect(within(card).getByText("Score")).toBeInTheDocument();
    expect(within(card).getByText("418")).toBeInTheDocument();
    expect(within(card).getByText("471")).toBeInTheDocument();
    expect(within(card).getByText("Winners rank 66, you rank 52")).toBeInTheDocument();
    fireEvent.keyDown(plot, { key: "ArrowLeft" });
    expect(within(container.querySelector(".c-radar__card") as HTMLElement).getByText("Shots")).toBeInTheDocument();
    fireEvent.keyDown(plot, { key: "Escape" });
    expect(container.querySelector(".c-radar__card")).toBeNull();
  });

  it("reads the open stat to screen readers and lists every stat", () => {
    render(<Radar label="Core" axes={core} />);
    fireEvent.keyDown(plotOf("Core"), { key: "ArrowRight" });
    expect(screen.getByText("Score: you 418, winners 471. Winners rank 66, you rank 52.", { selector: ".c-radar__live" })).toBeInTheDocument();
    const alt = screen.getByRole("list");
    expect(within(alt).getAllByRole("listitem")[2].textContent).toBe("Saves: you 1.6, winners 1.2. At or above the winners.");
  });

  it("falls back to ranks when a real value is missing", () => {
    const { container } = render(<Radar label="Core" axes={[{ key: "c", label: "Core", me: 56, winners: 65, unit: "average rank" }, ...core.slice(1)]} />);
    fireEvent.keyDown(plotOf("Core"), { key: "ArrowRight" });
    const card = container.querySelector(".c-radar__card") as HTMLElement;
    expect(within(card).getByText("rank 56")).toBeInTheDocument();
    expect(within(card).getByText("rank 65")).toBeInTheDocument();
  });

  it("dims an axis, explains it in the card, and reports clicks on labels", () => {
    const onAxis = vi.fn();
    const { container } = render(
      <Radar label="r" axes={[...core.slice(0, 2), { key: "m", label: "Mechanics", me: 50, winners: 50, dim: true, tip: "Nothing in Mechanics separated winners from losers." }]} onAxis={onAxis} />,
    );
    fireEvent.click(screen.getByText("Goals"));
    expect(onAxis).toHaveBeenCalledWith("goals");
    expect(screen.getByText("Mechanics")).toHaveClass("c-radar__dim");
    const plot = plotOf("r");
    ["ArrowRight", "ArrowRight", "ArrowRight"].forEach((key) => fireEvent.keyDown(plot, { key }));
    expect(within(container.querySelector(".c-radar__card") as HTMLElement).getByText("Nothing in Mechanics separated winners from losers.")).toBeInTheDocument();
  });
});

describe("takeaway", () => {
  it("praises when nothing is behind, and ignores dimmed and unmeasured axes", () => {
    expect(takeaway([{ key: "a", label: "A", me: 70, winners: 60 }, { key: "b", label: "B", me: 10, winners: 90, dim: true }, { key: "c", label: "C", me: null, winners: 80 }])).toBe(
      "At or above the winners everywhere",
    );
    expect(takeaway([])).toBeNull();
  });
});
