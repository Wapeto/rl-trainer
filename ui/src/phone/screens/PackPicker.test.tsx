import { render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { ApiError, type Api } from "@shared/api/client";
import { fakeApi } from "../../test/helpers";
import { PackPicker } from "./PackPicker";

function open(api: Api) {
  const onClose = vi.fn();
  const notify = vi.fn();
  render(<PackPicker open blockId={40} currentPackId={null} suggestion={null} api={api} notify={notify} onClose={onClose} />);
  return { onClose, notify };
}

describe("PackPicker", () => {
  it("closes once the pack is set", async () => {
    const { onClose } = open(fakeApi());
    await userEvent.click(await screen.findByRole("button", { name: /Aerial Shots - Pass/ }));
    expect(onClose).toHaveBeenCalled();
  });

  it("stays open when setting the pack fails", async () => {
    const api = fakeApi({ setPack: vi.fn().mockRejectedValue(new ApiError("conflict", "the activity is still being recorded", 409)) });
    const { onClose, notify } = open(api);
    await userEvent.click(await screen.findByRole("button", { name: /Aerial Shots - Pass/ }));
    expect(notify).toHaveBeenCalledWith("The activity is still being recorded", "error");
    expect(onClose).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog", { name: "Choose pack" })).toBeInTheDocument();
  });

  it("asks the PC for packs matching what is typed", async () => {
    const api = fakeApi();
    open(api);
    await screen.findByRole("button", { name: /Aerial Shots - Pass/ });
    await userEvent.type(screen.getByLabelText("Search packs"), "c7e09e0b");
    await waitFor(() => expect(api.packs).toHaveBeenLastCalledWith("c7e09e0b"));
    expect(await screen.findByRole("button", { name: /Aerial Shots - Pass/ })).toBeInTheDocument();
    await waitFor(() => expect(screen.queryByText("Backboard Reads")).toBeNull());
  });
});
