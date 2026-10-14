import { act, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { useState } from "react";
import { afterEach, describe, expect, it, vi } from "vitest";
import { MmrWheel } from "./MmrWheel";

function Harness({ start = 634, last = 634 as number | null, buzz = vi.fn() }) {
  const [v, setV] = useState<number | null>(start);
  return <MmrWheel label="MMR 1v1" value={v} last={last} onChange={setV} buzz={buzz} />;
}

function drag(el: Element, dy: number) {
  fireEvent.pointerDown(el, { clientY: 100, pointerId: 1 });
  fireEvent.pointerMove(el, { clientY: 100 + dy, pointerId: 1 });
  fireEvent.pointerUp(el, { clientY: 100 + dy, pointerId: 1 });
}


/** A fake clock and animation frames, so drags and flicks are exact. */
function fakeTime() {
  let clock = 0;
  let next = 0;
  const frames = new Map<number, FrameRequestCallback>();
  vi.spyOn(performance, "now").mockImplementation(() => clock);
  vi.stubGlobal("requestAnimationFrame", (cb: FrameRequestCallback) => {
    next += 1;
    frames.set(next, cb);
    return next;
  });
  vi.stubGlobal("cancelAnimationFrame", (id: number) => frames.delete(id));
  return {
    at: (ms: number) => {
      clock = ms;
    },
    pending: () => frames.size,
    runFrames: () => {
      for (let i = 0; i < 1000 && frames.size > 0; i++) {
        const batch = [...frames.values()];
        frames.clear();
        act(() => batch.forEach((cb) => cb(clock)));
      }
    },
  };
}

afterEach(() => {
  vi.restoreAllMocks();
  vi.unstubAllGlobals();
});

describe("MmrWheel", () => {
  it("raises the value when dragged down, one step per 14 px", () => {
    const buzz = vi.fn();
    render(<Harness buzz={buzz} />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    drag(wheel, 42);
    expect(wheel).toHaveAttribute("aria-valuenow", "637");
    expect(screen.getByText("+3")).toHaveClass("wheel__delta--up");
    expect(buzz).toHaveBeenCalledTimes(3);
  });

  it("lowers the value when dragged up and stays within 0–3000", () => {
    render(<Harness start={2} last={2} />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    drag(wheel, -70);
    expect(wheel).toHaveAttribute("aria-valuenow", "0");
  });

  it("steps with the keyboard", async () => {
    render(<Harness />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    wheel.focus();
    await userEvent.keyboard("{ArrowUp}{ArrowUp}{PageDown}");
    expect(wheel).toHaveAttribute("aria-valuenow", "626");
    expect(screen.getByText("−8")).toHaveClass("wheel__delta--down");
  });

  it("opens a number field on a tap and applies it", async () => {
    render(<Harness />);
    fireEvent.pointerDown(screen.getByRole("spinbutton"), {
      clientY: 100,
      pointerId: 1,
    });
    fireEvent.pointerUp(screen.getByRole("spinbutton"), {
      clientY: 101,
      pointerId: 1,
    });
    const input = screen.getByRole("textbox", { name: "MMR 1v1" });
    await userEvent.clear(input);
    await userEvent.type(input, "655{Enter}");
    expect(screen.getByRole("spinbutton", { name: "MMR 1v1" })).toHaveAttribute("aria-valuenow", "655");
  });

  it("starts as a number field when there is no MMR yet", () => {
    render(<Harness start={null as unknown as number} last={null} />);
    expect(screen.getByRole("textbox", { name: "MMR 1v1" })).toHaveValue("");
  });

  it("ignores a pointer moving over it without a press", () => {
    render(<Harness />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    fireEvent.pointerMove(wheel, { clientY: 100, pointerId: 1 });
    fireEvent.pointerMove(wheel, { clientY: 200, pointerId: 1 });
    expect(wheel).toHaveAttribute("aria-valuenow", "634");
  });

  it("does not coast when the finger rested before lifting", () => {
    const time = fakeTime();
    render(<Harness />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    fireEvent.pointerDown(wheel, { clientY: 100, pointerId: 1 });
    time.at(10);
    fireEvent.pointerMove(wheel, { clientY: 128, pointerId: 1 });
    time.at(310);
    fireEvent.pointerUp(wheel, { clientY: 128, pointerId: 1 });
    expect(time.pending()).toBe(0);
    expect(wheel).toHaveAttribute("aria-valuenow", "636");
  });

  it("keeps a fast flick within a few dozen steps", () => {
    const time = fakeTime();
    render(<Harness />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    fireEvent.pointerDown(wheel, { clientY: 100, pointerId: 1 });
    time.at(1);
    fireEvent.pointerMove(wheel, { clientY: 160, pointerId: 1 });
    time.at(2);
    fireEvent.pointerUp(wheel, { clientY: 160, pointerId: 1 });
    time.runFrames();
    const moved = Number(wheel.getAttribute("aria-valuenow")) - 634;
    expect(moved).toBeGreaterThan(4);
    expect(moved).toBeLessThanOrEqual(45);
  });

  it("only stops a coasting wheel when tapped, without opening the number field", () => {
    const time = fakeTime();
    render(<Harness />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    fireEvent.pointerDown(wheel, { clientY: 100, pointerId: 1 });
    time.at(1);
    fireEvent.pointerMove(wheel, { clientY: 160, pointerId: 1 });
    time.at(2);
    fireEvent.pointerUp(wheel, { clientY: 160, pointerId: 1 });
    fireEvent.pointerDown(wheel, { clientY: 300, pointerId: 2 });
    fireEvent.pointerUp(wheel, { clientY: 300, pointerId: 2 });
    expect(screen.queryByRole("textbox", { name: "MMR 1v1" })).toBeNull();
    expect(screen.getByRole("spinbutton", { name: "MMR 1v1" })).toBeInTheDocument();
  });

  it("stops coasting at a new touch", () => {
    const time = fakeTime();
    render(<Harness />);
    const wheel = screen.getByRole("spinbutton", { name: "MMR 1v1" });
    fireEvent.pointerDown(wheel, { clientY: 100, pointerId: 1 });
    time.at(1);
    fireEvent.pointerMove(wheel, { clientY: 160, pointerId: 1 });
    time.at(2);
    fireEvent.pointerUp(wheel, { clientY: 160, pointerId: 1 });
    fireEvent.pointerDown(wheel, { clientY: 300, pointerId: 2 });
    const held = wheel.getAttribute("aria-valuenow");
    time.runFrames();
    expect(wheel).toHaveAttribute("aria-valuenow", held!);
  });
});
