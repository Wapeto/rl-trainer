import { fireEvent, render, screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { Radar, radarPoint } from "./Radar";

const axes = [
  { key: "a", label: "Boost", me: 40, winners: 62, tip: "Boost: winners 62, losers 40, more is better" },
  { key: "b", label: "Touches", me: null, winners: 55 },
  { key: "c", label: "Movement", me: 70, winners: 58 },
];

describe("Radar", () => {
  it("puts 100 on the first axis straight up, at the radius", () => {
    expect(radarPoint(0, 3, 100)).toEqual([180, 50]);
    expect(radarPoint(0, 3, 0)).toEqual([180, 130]);
  });

  it("is a named figure with both shapes and a text alternative", () => {
    const { container } = render(<Radar label="You against the winners" axes={axes} />);
    const fig = screen.getByRole("figure", { name: "You against the winners" });
    expect(container.querySelector(".c-radar__winners")).not.toBeNull();
    expect(container.querySelector(".c-radar__me")).not.toBeNull();
    expect(within(fig).getAllByRole("listitem").map((li) => li.textContent)).toEqual([
      "Boost: you 40, winners 62 — Boost: winners 62, losers 40, more is better",
      "Touches: you —, winners 55",
      "Movement: you 70, winners 58",
    ]);
  });

  it("dims an axis and reports clicks on labels", () => {
    const onAxis = vi.fn();
    render(<Radar label="r" axes={[...axes.slice(0, 2), { ...axes[2], dim: true }]} onAxis={onAxis} />);
    fireEvent.click(screen.getByText(axes[1].label));
    expect(onAxis).toHaveBeenCalledWith(axes[1].key);
    expect(screen.getByText(axes[2].label)).toHaveClass("c-radar__dim");
  });

  it("gives a dimmed axis only its explanation in the text alternative", () => {
    const { container } = render(<Radar label="r" axes={[...axes.slice(0, 2), { key: "c", label: "Touches", me: 50, winners: 50, dim: true, tip: "nothing" }]} />);
    expect(container.querySelector(".c-radar__alt")!.lastChild!.textContent).toBe("Touches — nothing");
  });
});
