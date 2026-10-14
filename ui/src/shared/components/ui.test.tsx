import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Sheet } from "./Sheet";
import { Toast, useToast } from "./Toast";
import { Pill, StatRow } from "./ui";

describe("primitives", () => {
  it("renders a live pill and a stat row", () => {
    render(
      <>
        <Pill live>Training</Pill>
        <StatRow items={[{ label: "Made", value: "14/24" }, { label: "Streak", value: "2 ✓" }]} />
      </>,
    );
    expect(screen.getByText("Training")).toHaveClass("pill", "pill--live");
    expect(screen.getByText("14/24")).toHaveClass("stat__value");
  });

  it("closes the sheet from the backdrop but not from inside", async () => {
    const onClose = vi.fn();
    render(<Sheet open title="Choose pack" onClose={onClose}><p>inside</p></Sheet>);
    await userEvent.click(screen.getByText("inside"));
    expect(onClose).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("dialog").parentElement!);
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("renders nothing while closed", () => {
    render(<Sheet open={false} title="Choose pack" onClose={() => {}}>x</Sheet>);
    expect(screen.queryByRole("dialog")).toBeNull();
  });

  it("shows a toast and hides it after the timeout", () => {
    vi.useFakeTimers();
    function Demo() {
      const toast = useToast(3000);
      return (
        <>
          <button onClick={() => toast.show("MMR must be between 0 and 3000")}>go</button>
          <Toast toast={toast.current} />
        </>
      );
    }
    render(<Demo />);
    act(() => screen.getByText("go").click());
    expect(screen.getByRole("alert")).toHaveTextContent("MMR must be between 0 and 3000");
    act(() => vi.advanceTimersByTime(3000));
    expect(screen.queryByRole("alert")).toBeNull();
    vi.useRealTimers();
  });
});
