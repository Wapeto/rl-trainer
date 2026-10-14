import { render } from "@testing-library/react";
import { expect, it } from "vitest";
import { OutcomeDots } from "./OutcomeDots";

it("shows ten slots, newest on the right, with make, miss and skipped styles", () => {
  const attempts = [
    { id: 1, seq: 1, outcome: "make", skipped: false },
    { id: 2, seq: 2, outcome: "miss", skipped: true },
    { id: 3, seq: 3, outcome: "miss", skipped: false },
  ];
  const { container } = render(<OutcomeDots attempts={attempts} />);
  const dots = [...container.querySelectorAll(".dot")].map((d) => d.className);
  expect(dots).toHaveLength(10);
  expect(dots.slice(0, 7).every((c) => c === "dot dot--empty")).toBe(true);
  expect(dots.slice(7)).toEqual(["dot dot--make", "dot dot--skip", "dot dot--miss dot--newest"]);
});
