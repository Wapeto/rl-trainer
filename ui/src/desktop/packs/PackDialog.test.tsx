import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { PackRow } from "@shared/generated/types";
import type { DesktopApi } from "../api";
import { desktopFixtures as f } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { PackDialog, toInput } from "./PackDialog";

function open(pack: PackRow | null = null, api: DesktopApi = fakeDesktopApi()) {
  const onSaved = vi.fn();
  const onOpen = vi.fn();
  const onClose = vi.fn();
  render(<PackDialog api={api} notify={vi.fn()} pack={pack} onSaved={onSaved} onOpen={onOpen} onClose={onClose} />);
  return { api, onSaved, onOpen, onClose };
}

describe("PackDialog", () => {
  it("counts the name in characters, as the app does", () => {
    const fields = { code: "C7E0-9E0B-B739-A899", name: "🚀".repeat(80), target: "65", shots: "", tags: "", notes: "", goalie: false };
    expect(typeof toInput(fields)).toBe("object");
    expect(toInput({ ...fields, name: "🚀".repeat(81) })).toBe("The name needs 1 to 80 characters");
  });

  it("formats a code as you type it", async () => {
    open();
    await userEvent.type(screen.getByLabelText("Code"), "1111222233334444");
    expect(screen.getByLabelText("Code")).toHaveValue("1111-2222-3333-4444");
  });

  it("says what is wrong with a code once you leave the field", async () => {
    open();
    const code = screen.getByLabelText("Code");
    await userEvent.type(code, "c7e0 9e0b");
    expect(code).toHaveValue("C7E0-9E0B");
    expect(screen.queryByText("8 of 16 characters")).toBeNull();
    await userEvent.tab();
    expect(screen.getByText("8 of 16 characters")).toBeInTheDocument();
    await userEvent.clear(code);
    await userEvent.type(code, "C7E09E0BB739A8O9");
    expect(screen.getByText("Only 0–9 and A–F")).toBeInTheDocument();
  });

  it("offers to open the pack that already has this code", async () => {
    const { onOpen, onClose } = open();
    await userEvent.type(screen.getByLabelText("Code"), "c7e09e0bb739a899");
    expect(await screen.findByText(/Already saved as Aerial Shots - Pass/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Open it" }));
    expect(onClose).toHaveBeenCalled();
    expect(onOpen).toHaveBeenCalledWith(11);
  });

  it("adds a pack", async () => {
    const { api, onSaved } = open();
    await userEvent.type(screen.getByLabelText("Code"), "1111222233334444");
    await userEvent.type(screen.getByLabelText("Name"), "Fresh Pack");
    await userEvent.clear(screen.getByLabelText("Target make rate"));
    await userEvent.type(screen.getByLabelText("Target make rate"), "70");
    await userEvent.type(screen.getByLabelText("Shots"), "8");
    await userEvent.type(screen.getByLabelText("Tags"), " walls , saves,");
    await userEvent.click(screen.getByRole("button", { name: "Add pack" }));
    expect(api.addPack).toHaveBeenCalledWith({
      code: "1111-2222-3333-4444",
      name: "Fresh Pack",
      target_rate: 0.7,
      shot_count: 8,
      tags: ["walls", "saves"],
      notes: "",
      goalie: false,
    });
    expect(onSaved).toHaveBeenCalledWith(42);
  });

  it("marks a goalie pack, where a shot is made unless the ball goes in", async () => {
    const { api } = open({ ...f.packList[0].pack, goalie: false });
    const goalie = screen.getByRole("switch", { name: "Goalie pack" });
    expect(goalie).toHaveAttribute("aria-checked", "false");
    expect(screen.getByText("A shot counts as made unless the ball goes in.")).toBeInTheDocument();
    await userEvent.click(goalie);
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.updatePack).toHaveBeenCalledWith(f.packList[0].pack.id, expect.objectContaining({ goalie: true }));
  });

  it("checks the fields before sending", async () => {
    const { api } = open();
    await userEvent.type(screen.getByLabelText("Code"), "1111222233334444");
    await userEvent.click(screen.getByRole("button", { name: "Add pack" }));
    expect(screen.getByRole("alert")).toHaveTextContent("The name needs 1 to 80 characters");
    await userEvent.type(screen.getByLabelText("Name"), "X");
    await userEvent.clear(screen.getByLabelText("Target make rate"));
    await userEvent.type(screen.getByLabelText("Target make rate"), "0");
    await userEvent.click(screen.getByRole("button", { name: "Add pack" }));
    expect(screen.getByRole("alert")).toHaveTextContent("The target is a whole percentage from 1 to 100");
    expect(api.addPack).not.toHaveBeenCalled();
  });

  it("edits a pack and shows the app's refusal", async () => {
    const api = fakeDesktopApi({
      updatePack: vi.fn().mockRejectedValue(new Error("a pack with the code 0000-0000-0000-0000 already exists")),
    });
    const { onClose } = open(f.packList[0].pack, api);
    expect(screen.getByLabelText("Name")).toHaveValue("Aerial Shots - Pass");
    expect(screen.getByLabelText("Code")).toHaveValue("C7E0-9E0B-B739-A899");
    expect(screen.getByLabelText("Shots")).toHaveValue("10");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.updatePack).toHaveBeenCalledWith(
      11,
      expect.objectContaining({ name: "Aerial Shots - Pass", target_rate: 0.65, shot_count: 10 }),
    );
    expect(await screen.findByText("A pack with the code 0000-0000-0000-0000 already exists")).toBeInTheDocument();
    expect(onClose).not.toHaveBeenCalled();
  });
});
