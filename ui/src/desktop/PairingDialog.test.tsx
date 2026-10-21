import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { Settings } from "@shared/generated/types";
import type { DesktopApi } from "./api";
import { desktopFixtures as f } from "./fixtures";
import { PairingDialog } from "./PairingDialog";
import { fakeDesktopApi } from "./test/fakeApi";

function renderDialog(api: DesktopApi = fakeDesktopApi()) {
  const onClose = vi.fn();
  render(<PairingDialog api={api} notify={vi.fn()} phones={0} onClose={onClose} />);
  return { api, onClose };
}

describe("PairingDialog", () => {
  it("shows the QR code and the masked link when the phone page is on", async () => {
    renderDialog();
    expect(await screen.findByRole("img", { name: "QR code to pair your phone" })).toBeInTheDocument();
    expect(screen.getByText("http://192.168.1.66:49321/#t=••••••••")).toBeInTheDocument();
    expect(screen.getByText("No phone connected")).toBeInTheDocument();
  });

  it("offers to turn the phone page on when it is off, then shows the code", async () => {
    const off: Settings = { ...f.settings, phone_enabled: false };
    const settings = vi.fn().mockResolvedValueOnce(off).mockResolvedValue(f.settings);
    const { api } = renderDialog(fakeDesktopApi({ settings }));
    expect(screen.queryByRole("img", { name: "QR code to pair your phone" })).toBeNull();
    await userEvent.click(await screen.findByRole("button", { name: "Turn on phone page" }));
    expect(api.enablePhone).toHaveBeenCalledTimes(1);
    expect(await screen.findByRole("img", { name: "QR code to pair your phone" })).toBeInTheDocument();
  });

  it("closes on Escape", async () => {
    const { onClose } = renderDialog();
    await screen.findByRole("img", { name: "QR code to pair your phone" });
    await userEvent.keyboard("{Escape}");
    expect(onClose).toHaveBeenCalled();
  });
});
