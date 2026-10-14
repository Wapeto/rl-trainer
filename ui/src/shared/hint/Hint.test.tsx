import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { Gloss } from "../glossary";
import { Hint } from "./Hint";

const WHIFF: Gloss = { title: "What a whiff is", text: "Times you missed." };
const PADS: Gloss = { title: "Big pads", text: "Pads you took." };

afterEach(() => vi.unstubAllGlobals());

function coarse() {
  vi.stubGlobal("matchMedia", (query: string) => ({ matches: query === "(pointer: coarse)", media: query }));
}

describe("Hint", () => {
  it("opens on hover after a short delay and closes on leave", async () => {
    render(<Hint gloss={WHIFF}>Whiffs</Hint>);
    await userEvent.hover(screen.getByText("Whiffs"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent("What a whiff isTimes you missed.");
    await userEvent.unhover(screen.getByText("Whiffs"));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("opens on keyboard focus, describes its label, and closes on Escape", async () => {
    render(<Hint gloss={WHIFF}>Whiffs</Hint>);
    await userEvent.tab();
    const tip = screen.getByRole("tooltip");
    expect(screen.getByText("Whiffs")).toHaveAttribute("aria-describedby", tip.id);
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("keeps only one hint open", async () => {
    render(
      <>
        <Hint gloss={WHIFF}>Whiffs</Hint>
        <Hint gloss={PADS}>Pads</Hint>
      </>,
    );
    await userEvent.tab();
    await userEvent.tab();
    const tips = screen.getAllByRole("tooltip");
    expect(tips).toHaveLength(1);
    expect(tips[0]).toHaveTextContent("Pads you took.");
  });

  it("toggles on tap on a phone and closes on a tap elsewhere", async () => {
    coarse();
    render(
      <>
        <Hint gloss={WHIFF}>Whiffs</Hint>
        <p>elsewhere</p>
      </>,
    );
    await userEvent.click(screen.getByText("Whiffs"));
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    await userEvent.click(screen.getByText("elsewhere"));
    expect(screen.queryByRole("tooltip")).toBeNull();
    await userEvent.click(screen.getByText("Whiffs"));
    await userEvent.click(screen.getByText("Whiffs"));
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("leaves a wrapped control's click alone and gets out of the way when it is clicked", async () => {
    const onClick = vi.fn();
    render(
      <Hint gloss={WHIFF} control>
        <button type="button" onClick={onClick}>
          Go
        </button>
      </Hint>,
    );
    await userEvent.hover(screen.getByRole("button", { name: "Go" }));
    expect(await screen.findByRole("tooltip")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Go" }));
    expect(onClick).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("tooltip")).toBeNull();
    expect(screen.getByText("Go").closest(".hint")).not.toHaveAttribute("tabindex");
  });

  it("describes a wrapped control reached with the keyboard", async () => {
    render(
      <Hint gloss={WHIFF} control>
        <button type="button">Go</button>
      </Hint>,
    );
    await userEvent.tab();
    const tip = screen.getByRole("tooltip");
    expect(screen.getByRole("button", { name: "Go" })).toHaveAttribute("aria-describedby", tip.id);
  });

  it("closes when its label scrolls out of view", async () => {
    render(<Hint gloss={WHIFF}>Whiffs</Hint>);
    await userEvent.tab();
    expect(screen.getByRole("tooltip")).toBeInTheDocument();
    const label = screen.getByText("Whiffs");
    label.getBoundingClientRect = () => ({ left: 0, top: -60, bottom: -40, right: 50, width: 50, height: 20, x: 0, y: -60, toJSON: () => ({}) });
    fireEvent.scroll(window);
    expect(screen.queryByRole("tooltip")).toBeNull();
  });

  it("renders just its children without a gloss", () => {
    const { container } = render(<Hint gloss={null}>Plain</Hint>);
    expect(container.querySelector(".hint")).toBeNull();
    expect(screen.getByText("Plain")).toBeInTheDocument();
  });
});
