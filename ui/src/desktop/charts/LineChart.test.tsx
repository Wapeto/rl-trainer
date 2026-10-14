import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { LineChart, type ChartSeries } from "./LineChart";
import { Sparkline } from "./Sparkline";

const series: ChartSeries[] = [
  {
    label: "Make rate",
    color: "#6fe3a9",
    points: [
      { at: 1_000, value: 50, sessionId: 1, bar: 20, tip: ["50%", "1 Oct"] },
      { at: 2_000, value: 75, sessionId: 2, bar: 8, hollow: true, tip: ["75%", "8 Oct"] },
      { at: 3_000, value: 80, sessionId: 3, bar: 30, tip: ["80%", "15 Oct"] },
    ],
  },
];

describe("LineChart", () => {
  it("draws one point per session, hollow ones apart, with bars and the reference line", () => {
    const { container } = render(<LineChart label="Make rate" series={series} height={200} reference={65} unit="%" />);
    expect(screen.getByRole("img", { name: "Make rate" })).toBeInTheDocument();
    expect(container.querySelectorAll(".c-pt")).toHaveLength(3);
    expect(container.querySelectorAll(".c-pt--hollow")).toHaveLength(1);
    expect(container.querySelectorAll(".c-bar")).toHaveLength(3);
    expect(container.querySelector(".c-ref")).not.toBeNull();
  });

  it("shows the tooltip on hover and opens the session on click", () => {
    const onOpenSession = vi.fn();
    render(<LineChart label="Make rate" series={series} height={200} onOpenSession={onOpenSession} />);
    const point = screen.getByLabelText("75% · 8 Oct");
    fireEvent.mouseEnter(point);
    expect(screen.getByRole("tooltip")).toHaveTextContent("75%8 Oct");
    fireEvent.click(point);
    expect(onOpenSession).toHaveBeenCalledWith(2);
  });

  it("draws the average line only through points that have one", () => {
    const { container } = render(
      <LineChart label="Make rate" series={series} height={200} average={[0.5, null, 0.65].map((v) => (v == null ? null : v * 100))} />,
    );
    expect(container.querySelector(".c-avg")?.getAttribute("points")?.split(" ")).toHaveLength(2);
  });

  it("says when there is nothing to draw", () => {
    render(<LineChart label="Goals" series={[{ label: "Goals", color: "#fff", points: [] }]} height={80} />);
    expect(screen.getByText("No data yet")).toBeInTheDocument();
  });
});

describe("Sparkline", () => {
  it("needs two values to draw a line", () => {
    const { container, rerender } = render(<Sparkline values={[3]} color="#fff" />);
    expect(container.querySelector("polyline")).toBeNull();
    rerender(<Sparkline values={[3, 5, 4]} color="#fff" />);
    expect(container.querySelector("polyline")).not.toBeNull();
  });
});
