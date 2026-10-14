import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import { Ring } from "./Ring";

const C_HERO = 2 * Math.PI * 53;

describe("Ring", () => {
  it("fills the arc to the value and marks the target", () => {
    render(<Ring value={0.58} target={0.65} tone="amber" size="hero" label="Make rate 58%">58</Ring>);
    const arc = screen.getByTestId("ring-arc");
    const [filled, total] = arc.getAttribute("stroke-dasharray")!.split(" ").map(Number);
    expect(filled / total).toBeCloseTo(0.58, 2);
    expect(total).toBeCloseTo(C_HERO, 0);
    expect(screen.getByTestId("ring-target").getAttribute("transform")).toBe("rotate(234 60 60)");
    expect(screen.getByRole("img", { name: "Make rate 58%" })).toHaveTextContent("58");
  });

  it("draws no arc at zero and clamps above one", () => {
    const { rerender } = render(<Ring value={0} tone="blue" size="tile" label="x" />);
    expect(screen.queryByTestId("ring-arc")).toBeNull();
    rerender(<Ring value={1.7} tone="blue" size="tile" label="x" />);
    const [filled, total] = screen.getByTestId("ring-arc").getAttribute("stroke-dasharray")!.split(" ").map(Number);
    expect(filled).toBeCloseTo(total, 0);
  });

  it("has a mini size for lists", () => {
    render(<Ring value={0.5} tone="green" size="mini" label="Make rate 50%" />);
    expect(screen.getByRole("img", { name: "Make rate 50%" })).toHaveStyle({ width: "30px" });
  });
});
