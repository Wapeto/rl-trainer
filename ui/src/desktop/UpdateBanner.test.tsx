import { act, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, describe, expect, it, vi } from "vitest";
import { fixtures } from "@shared/fixtures";
import { DesktopApp, UPDATE_CHECK_DELAY_MS } from "./App";
import { UpdateBanner } from "./UpdateBanner";
import { DESKTOP_NOW } from "./fixtures";
import { fakeDesktopApi, feedOf, memoryStorage } from "./test/fakeApi";

const update = { version: "0.2.1", notes: null };

describe("UpdateBanner", () => {
  afterEach(() => vi.useRealTimers());

  it("installs at once when no game is running", async () => {
    const api = fakeDesktopApi();
    render(<UpdateBanner api={api} update={update} gameRunning={false} notify={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(api.installUpdate).toHaveBeenCalledTimes(1);
  });

  it("warns that the session ends when the game is running", async () => {
    const api = fakeDesktopApi();
    render(<UpdateBanner api={api} update={update} gameRunning notify={vi.fn()} />);
    await userEvent.click(screen.getByRole("button", { name: "Install and restart" }));
    expect(api.installUpdate).not.toHaveBeenCalled();
    expect(screen.getByText(/current session ends/)).toBeInTheDocument();
  });

  it("checks once after the start delay and shows the banner", async () => {
    vi.useFakeTimers();
    const api = fakeDesktopApi({ checkUpdate: vi.fn().mockResolvedValue(update) });
    render(<DesktopApp api={api} feed={feedOf(fixtures.idle)} now={DESKTOP_NOW} storage={memoryStorage()} />);
    expect(api.checkUpdate).not.toHaveBeenCalled();
    await act(() => vi.advanceTimersByTimeAsync(UPDATE_CHECK_DELAY_MS));
    expect(api.checkUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getByText("Version 0.2.1 is ready")).toBeInTheDocument();
  });
});
