import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { FIXTURE_NOW, fixtures } from "@shared/fixtures";
import type { AppSnapshot } from "@shared/generated/types";
import { fakeApi } from "../../test/helpers";
import type { ScreenProps } from "../screenProps";
import { Training, targetChip } from "./Training";

function props(snap: AppSnapshot, over: Partial<ScreenProps> = {}): ScreenProps {
  return { snap, conn: "live", now: FIXTURE_NOW, api: fakeApi(), notify: vi.fn(), ...over };
}

describe("Training", () => {
  it("shows the make rate against the target in amber below it", () => {
    const { container } = render(<Training {...props(fixtures.trainingBelow)} />);
    expect(container.firstChild).toHaveClass("tone-amber");
    expect(screen.getByRole("heading", { name: "Aerial Shots - Pass" })).toBeInTheDocument();
    expect(screen.getByText("C7E0-9E0B-B739-A899")).toBeInTheDocument();
    expect(screen.getByRole("img", { name: "Make rate 58%, target 65%" })).toBeInTheDocument();
    expect(screen.getByText("−7 vs target 65%")).toBeInTheDocument();
    expect(screen.getByText("14/24")).toBeInTheDocument();
    expect(screen.getByText("8:32")).toBeInTheDocument();
    expect(screen.getByText("2 ✓")).toBeInTheDocument();
    expect(screen.getByText("Session 1:04")).toBeInTheDocument();
  });

  it("turns green at or above the target", () => {
    const { container } = render(<Training {...props(fixtures.trainingAbove)} />);
    expect(container.firstChild).toHaveClass("tone-green");
    expect(screen.getByText("+6 above target 65%")).toBeInTheDocument();
  });

  it("words the target chip", () => {
    expect(targetChip(null, 0.65)).toBe("Target 65%");
    expect(targetChip(0.65, 0.65)).toBe("On target 65%");
  });

  it("skips and un-skips the last attempt", async () => {
    const api = fakeApi();
    const { rerender } = render(<Training {...props(fixtures.trainingBelow, { api })} />);
    await userEvent.click(screen.getByRole("button", { name: "Skip last" }));
    expect(api.setSkipped).toHaveBeenCalledWith(509, true);
    const skipped = structuredClone(fixtures.trainingBelow);
    skipped.live.block!.training!.recent.at(-1)!.skipped = true;
    rerender(<Training {...props(skipped, { api })} />);
    await userEvent.click(screen.getByRole("button", { name: "Undo skip" }));
    expect(api.setSkipped).toHaveBeenLastCalledWith(509, false);
  });

  it("offers the last pack in one tap when none is set", async () => {
    const api = fakeApi();
    render(<Training {...props(fixtures.trainingSuggest, { api })} />);
    expect(screen.getByRole("heading", { name: "No pack set" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Skip last" })).toBeDisabled();
    await userEvent.click(screen.getByRole("button", { name: "Still Aerial Shots - Pass?" }));
    expect(api.setPack).toHaveBeenCalledWith(40, 11);
  });

  it("reports a refused action in the toast", async () => {
    const notify = vi.fn();
    const { ApiError } = await import("@shared/api/client");
    const api = fakeApi({
      setSkipped: vi.fn().mockRejectedValue(new ApiError("conflict", "that attempt is from a session that already ended", 409)),
    });
    render(<Training {...props(fixtures.trainingBelow, { api, notify })} />);
    await userEvent.click(screen.getByRole("button", { name: "Skip last" }));
    expect(notify).toHaveBeenCalledWith("That attempt is from a session that already ended", "error");
  });
});

describe("PackPicker", () => {
  async function openPicker(api = fakeApi()) {
    const utils = render(<Training {...props(fixtures.trainingBelow, { api })} />);
    await userEvent.click(screen.getByRole("button", { name: "Change pack" }));
    const dialog = await screen.findByRole("dialog", { name: "Choose pack" });
    await within(dialog).findByText("Aerial Off Wall");
    return { ...utils, api, dialog };
  }

  it("lists packs with their make rate and filters by name, code or tag", async () => {
    const { dialog } = await openPicker();
    expect(within(dialog).getByText("64%")).toBeInTheDocument();
    await userEvent.type(within(dialog).getByLabelText("Search packs"), "backboard");
    await waitFor(() => expect(within(dialog).getByText("Backboard Reads")).toBeInTheDocument());
    await waitFor(() => expect(within(dialog).queryByText("Aerial Off Wall")).toBeNull());
    await userEvent.clear(within(dialog).getByLabelText("Search packs"));
    await userEvent.type(within(dialog).getByLabelText("Search packs"), "c7e0");
    await waitFor(() => expect(within(dialog).getByText("Aerial Shots - Pass")).toBeInTheDocument());
    await waitFor(() => expect(within(dialog).queryByText("Consistency")).toBeNull());
  });

  it("sets the pack, or none, and closes", async () => {
    const { api, dialog } = await openPicker();
    await userEvent.click(within(dialog).getByText("Unassigned"));
    expect(api.setPack).toHaveBeenCalledWith(40, null);
    await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  });

  it("uses the block shown at tap time, not the one it was opened on", async () => {
    const { api, dialog, rerender } = await openPicker();
    const split = structuredClone(fixtures.trainingBelow);
    split.live.block!.id = 41;
    rerender(<Training {...props(split, { api })} />);
    await userEvent.click(within(screen.getByRole("dialog")).getByText("Consistency"));
    expect(api.setPack).toHaveBeenCalledWith(41, 6);
    await waitFor(() => expect(dialog).not.toBeInTheDocument()); // closed after choosing
  });
});
