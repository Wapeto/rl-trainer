import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { AppSnapshot } from "@shared/generated/types";
import { fixtures } from "@shared/fixtures";
import type { DesktopApi } from "../api";
import { DESKTOP_NOW, desktopFixtures as f } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { GetStarted } from "./GetStarted";

const off = { ...fixtures.trainingBelow, phone_clients: 0, stats_api: { state: "disabled", ini: "C:/x.ini" } } as AppSnapshot;

function renderCard(api: DesktopApi, snapshot: AppSnapshot | null, onOpenSettings = vi.fn()) {
  render(
    <GetStarted api={api} snapshot={snapshot} now={DESKTOP_NOW} notify={vi.fn()} reload="r" onChanged={vi.fn()} onOpenSettings={onOpenSettings} />,
  );
  return onOpenSettings;
}

describe("GetStarted", () => {
  it("is hidden when the Stats API works and an account is detected", async () => {
    const api = fakeDesktopApi();
    renderCard(api, fixtures.trainingBelow);
    await vi.waitFor(() => expect(api.profiles).toHaveBeenCalled());
    expect(screen.queryByRole("region", { name: "Get started" })).toBeNull();
  });

  it("fixes the Stats API and shows the missing account", async () => {
    const api = fakeDesktopApi({ profiles: vi.fn().mockResolvedValue([]) });
    renderCard(api, off);
    expect(await screen.findByText(/Open free play/)).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Fix…" }));
    expect(api.fixStatsApi).toHaveBeenCalled();
  });

  it("turns on the phone page and can be dismissed", async () => {
    const api = fakeDesktopApi({ settings: vi.fn().mockResolvedValue({ ...f.settings, phone_enabled: false }) });
    renderCard(api, off);
    await userEvent.click(await screen.findByRole("button", { name: "Turn on phone page" }));
    expect(api.enablePhone).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Dismiss" }));
    expect(api.updateSettings).toHaveBeenCalledWith({ setup_dismissed: true });
  });

  it("sends the user to Settings when the game is not found and the phone is not paired", async () => {
    const api = fakeDesktopApi();
    const notFound = { ...off, stats_api: { state: "install_not_found" } } as AppSnapshot;
    const open = renderCard(api, notFound);
    expect(await screen.findByText(/set its folder in Settings/)).toBeInTheDocument();
    expect(screen.getByText(/Scan the QR code/)).toBeInTheDocument();
    const buttons = screen.getAllByRole("button", { name: "Open Settings" });
    expect(buttons).toHaveLength(2); // Stats API and phone; the account is done
    await userEvent.click(buttons[0]);
    expect(open).toHaveBeenCalled();
  });

  it("stays hidden once dismissed", async () => {
    const api = fakeDesktopApi({ settings: vi.fn().mockResolvedValue({ ...f.settings, setup_dismissed: true }) });
    renderCard(api, off);
    await vi.waitFor(() => expect(api.settings).toHaveBeenCalled());
    expect(screen.queryByRole("region", { name: "Get started" })).toBeNull();
  });
});
