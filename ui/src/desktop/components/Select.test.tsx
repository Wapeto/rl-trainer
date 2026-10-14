import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Select } from "./Select";

const OPTIONS = [
  { value: "played", label: "Played in range" },
  { value: "change", label: "Biggest change" },
  { value: "name", label: "Name" },
];

describe("Select", () => {
  it("shows the current choice and lists the others in the window's own menu", async () => {
    const onChange = vi.fn();
    render(<Select label="Sort packs" value="played" options={OPTIONS} onChange={onChange} />);
    const button = screen.getByRole("button", { name: "Sort packs" });
    expect(button).toHaveTextContent("Played in range");
    expect(screen.queryByRole("listbox")).toBeNull();
    await userEvent.click(button);
    const list = screen.getByRole("listbox", { name: "Sort packs" });
    expect(list).toHaveClass("d-menu");
    expect(screen.getByRole("option", { name: "Played in range" })).toHaveAttribute("aria-selected", "true");
    await userEvent.click(screen.getByRole("option", { name: "Name" }));
    expect(onChange).toHaveBeenCalledWith("name");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("closes on Escape or a click elsewhere without changing anything", async () => {
    const onChange = vi.fn();
    render(
      <>
        <Select label="Sort packs" value="played" options={OPTIONS} onChange={onChange} />
        <p>elsewhere</p>
      </>,
    );
    await userEvent.click(screen.getByRole("button", { name: "Sort packs" }));
    await userEvent.keyboard("{Escape}");
    expect(screen.queryByRole("listbox")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Sort packs" }));
    await userEvent.click(screen.getByText("elsewhere"));
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(onChange).not.toHaveBeenCalled();
  });
});
