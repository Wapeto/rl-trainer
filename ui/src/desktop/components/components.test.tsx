import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { Confirm, Dialog } from "./Dialog";
import { MenuButton } from "./Menu";

describe("Dialog", () => {
  it("closes on Escape and on a click outside", async () => {
    const onClose = vi.fn();
    render(
      <Dialog title="Which pack was this?" onClose={onClose}>
        <p>body</p>
      </Dialog>,
    );
    expect(screen.getByRole("dialog", { name: "Which pack was this?" })).toBeInTheDocument();
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalledTimes(1);
  });

  it("runs the confirm action once and disables the button meanwhile", async () => {
    let finish: () => void = () => {};
    const onConfirm = vi.fn(() => new Promise<void>((resolve) => (finish = resolve)));
    render(<Confirm title="Delete this session?" body="Gone." confirm="Delete" danger onConfirm={onConfirm} onClose={vi.fn()} />);
    const button = screen.getByRole("button", { name: "Delete" });
    await userEvent.click(button);
    expect(button).toBeDisabled();
    await userEvent.click(button);
    expect(onConfirm).toHaveBeenCalledTimes(1);
    finish();
  });
});

describe("MenuButton", () => {
  it("opens, runs an item and closes; disabled items do nothing", async () => {
    const merge = vi.fn();
    const next = vi.fn();
    render(
      <MenuButton
        label="Session actions"
        items={[
          { label: "Merge with previous session", onSelect: merge },
          { label: "Merge with next session", onSelect: next, disabled: true },
        ]}
      />,
    );
    await userEvent.click(screen.getByRole("button", { name: "Session actions" }));
    expect(screen.getByRole("menuitem", { name: "Merge with next session" })).toBeDisabled();
    await userEvent.click(screen.getByRole("menuitem", { name: "Merge with previous session" }));
    expect(merge).toHaveBeenCalled();
    expect(screen.queryByRole("menu")).toBeNull();
  });
});
