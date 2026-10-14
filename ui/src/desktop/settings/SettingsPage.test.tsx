import { act, render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import { fixtures } from "@shared/fixtures";
import type { AppSnapshot } from "@shared/generated/types";
import type { DesktopApi } from "../api";
import { DESKTOP_NOW, desktopFixtures as f } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { SettingsPage } from "./SettingsPage";
import { backupLabel, maskToken } from "./sections";

function renderPage(api: DesktopApi = fakeDesktopApi(), snapshot: AppSnapshot = fixtures.trainingBelow) {
  render(<SettingsPage api={api} snapshot={snapshot} now={DESKTOP_NOW} notify={vi.fn()} reload="r" onChanged={vi.fn()} />);
  return { api };
}

describe("SettingsPage", () => {
  it("shows the phone pairing with the token hidden", async () => {
    renderPage();
    expect(await screen.findByRole("img", { name: "QR code to pair your phone" })).toBeInTheDocument();
    expect(screen.getByText("http://192.168.1.66:49321/#t=••••••••")).toBeInTheDocument();
    expect(screen.getByText("1 phone connected")).toBeInTheDocument();
    expect(screen.getByRole("switch", { name: "Phone page" })).toHaveAttribute("aria-checked", "true");
    expect(maskToken("http://x:1/#t=secret")).toBe("http://x:1/#t=••••••••");
  });

  it("makes a new pairing code and turns the phone page off", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "New pairing code" }));
    expect(api.regenerateToken).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("switch", { name: "Phone page" }));
    expect(api.disablePhone).toHaveBeenCalled();
  });

  it("shows the app's refusal under the field", async () => {
    const api = fakeDesktopApi({
      updateSettings: vi.fn().mockRejectedValue(new Error("port must be between 1024 and 65535")),
    });
    renderPage(api);
    const port = await screen.findByLabelText("Port");
    await userEvent.clear(port);
    await userEvent.type(port, "80");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.updateSettings).toHaveBeenCalledWith({ phone_port: 80 });
    expect(await screen.findByText("Port must be between 1024 and 65535")).toBeInTheDocument();
  });

  it("saves the session timeout", async () => {
    const { api } = renderPage();
    const timeout = await screen.findByLabelText("Session timeout");
    expect(timeout).toHaveValue("30");
    await userEvent.clear(timeout);
    await userEvent.type(timeout, "45");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.updateSettings).toHaveBeenCalledWith({ session_timeout_min: 45 });
  });

  it("offers a fix when the Stats API is off", async () => {
    const snapshot = { ...fixtures.trainingBelow, stats_api: { state: "disabled", ini: "C:/x.ini" } } as AppSnapshot;
    const { api } = renderPage(fakeDesktopApi(), snapshot);
    expect(await screen.findByText("Turned off in the game's settings file")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Fix…" }));
    expect(api.fixStatsApi).toHaveBeenCalled();
  });

  it("sets the Rocket League folder", async () => {
    const { api } = renderPage();
    await userEvent.type(await screen.findByLabelText("Rocket League folder"), "D:\\Games\\rocketleague");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.updateSettings).toHaveBeenCalledWith({ install_dir: "D:\\Games\\rocketleague" });
  });

  it("lists backups and restores one after asking", async () => {
    const { api } = renderPage();
    expect(await screen.findByText(/before merge/)).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: "Restore" })).toHaveLength(3);
    await userEvent.click(screen.getByRole("button", { name: "Show all (4)" }));
    expect(screen.getAllByRole("button", { name: "Restore" })).toHaveLength(4);
    await userEvent.click(screen.getAllByRole("button", { name: "Restore" })[0]);
    const dialog = screen.getByRole("dialog", { name: "Restore this backup?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Restore" }));
    expect(api.restoreBackup).toHaveBeenCalledWith("pre-edit-20261008-215800-120-merge.db");
    expect(backupLabel(f.backups[1])).toBe("daily");
  });

  it("switches the account from the app's own menu", async () => {
    const second = { display_name: "Alt", is_active: false, primary_id: "Steam|alt|0" };
    const api = fakeDesktopApi({ profiles: vi.fn().mockResolvedValue([...f.profiles, second]) });
    renderPage(api);
    await userEvent.click(await screen.findByRole("button", { name: "Account" }));
    await userEvent.click(screen.getByRole("option", { name: "Alt" }));
    expect(api.setActiveProfile).toHaveBeenCalledWith("Steam|alt|0");
  });

  it("saves an in-game name as the account and offers a restart", async () => {
    const { api } = renderPage();
    expect(await screen.findByText(/Tracking .* \(Epic\)/)).toBeInTheDocument();
    await userEvent.type(screen.getByLabelText("In-game name or Epic account ID"), "  MyName ");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.updateSettings).toHaveBeenCalledWith({ me_hint: "MyName" });
    await userEvent.click(await screen.findByRole("button", { name: "Restart now" }));
    expect(api.restartApp).toHaveBeenCalled();
  });

  it("says when no account is detected yet", async () => {
    renderPage(fakeDesktopApi({ profiles: vi.fn().mockResolvedValue([]) }));
    expect(await screen.findByText(/Not detected yet/)).toBeInTheDocument();
  });

  it("shows autostart, the version and the data folder", async () => {
    const { api } = renderPage();
    expect(await screen.findByRole("switch", { name: "Start with Windows" })).toHaveAttribute("aria-checked", "true");
    expect(screen.getByText(/RL Trainer 0\.1\.0/)).toBeInTheDocument();
    expect(screen.queryByLabelText("Account")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Open folder" }));
    expect(api.openDataFolder).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Releases" }));
    expect(api.openReleases).toHaveBeenCalled();
  });

  it("restarts the app after asking", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Restart" }));
    const dialog = screen.getByRole("dialog", { name: "Restart RL Trainer?" });
    expect(api.restartApp).not.toHaveBeenCalled();
    await userEvent.click(within(dialog).getByRole("button", { name: "Restart" }));
    expect(api.restartApp).toHaveBeenCalledTimes(1);
  });

  it("shows the restart in progress, and lets it be tried again when it fails", async () => {
    const { api } = renderPage();
    let fail: (e: Error) => void = () => {};
    vi.mocked(api.restartApp).mockReturnValueOnce(new Promise((_, reject) => (fail = reject)));
    await userEvent.click(await screen.findByRole("button", { name: "Restart" }));
    const dialog = screen.getByRole("dialog", { name: "Restart RL Trainer?" });
    await userEvent.click(within(dialog).getByRole("button", { name: "Restart" }));
    expect(within(dialog).getByRole("button", { name: "Restarting…" })).toBeDisabled();
    await act(async () => fail(new Error("Rebuild failed")));
    expect(within(dialog).getByRole("button", { name: "Restart" })).toBeEnabled();
  });

  it("quits the app after asking, and not when the question is cancelled", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Quit" }));
    await userEvent.click(within(screen.getByRole("dialog", { name: "Quit RL Trainer?" })).getByRole("button", { name: "Cancel" }));
    expect(api.quitApp).not.toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "Quit" }));
    await userEvent.click(within(screen.getByRole("dialog", { name: "Quit RL Trainer?" })).getByRole("button", { name: "Quit" }));
    expect(api.quitApp).toHaveBeenCalledTimes(1);
  });

  it("hides Quit and Restart where the window cannot control the app", async () => {
    renderPage(fakeDesktopApi({ canControlApp: false }));
    expect(await screen.findByText(/RL Trainer 0\.1\.0/)).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Restart" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Quit" })).toBeNull();
  });

  it("hides autostart where the window cannot control it", async () => {
    renderPage(fakeDesktopApi({ autostart: vi.fn().mockResolvedValue(null) }));
    expect(await screen.findByText(/RL Trainer 0\.1\.0/)).toBeInTheDocument();
    expect(screen.queryByRole("switch", { name: "Start with Windows" })).toBeNull();
  });

  it("shows the watched replay folder and lets the user set another", async () => {
    const api = fakeDesktopApi();
    renderPage(api);
    const section = await screen.findByRole("region", { name: "Replays" });
    expect(within(section).getByText(/Watching C:\\Users\\me\\OneDrive/)).toBeInTheDocument();
    expect(await within(section).findByText("4 imported · 1 couldn't be read")).toBeInTheDocument();
    await userEvent.type(within(section).getByLabelText("Replay folder"), "D:\\Replays");
    await userEvent.click(within(section).getByRole("button", { name: "Save" }));
    expect(api.updateSettings).toHaveBeenCalledWith({ replay_dir: "D:\\Replays" });
  });

  it("warns when the latest replays could not be read", async () => {
    const status = { ...f.replayStatus, counts: { ...f.replayStatus.counts, format_warning: true } };
    renderPage(fakeDesktopApi({ replayStatus: vi.fn().mockResolvedValue(status) }));
    expect(await screen.findByText(/may have changed its replay format/)).toBeInTheDocument();
  });
});
