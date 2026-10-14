import { fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { DesktopApi } from "../api";
import { DESKTOP_NOW } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { FeedbackPage } from "./FeedbackPage";

function renderPage(api: DesktopApi = fakeDesktopApi()) {
  const notify = vi.fn();
  render(<FeedbackPage api={api} snapshot={null} now={DESKTOP_NOW} notify={notify} reload="r" onChanged={vi.fn()} />);
  return { api, notify };
}

const png = () => new File([new Uint8Array([0x89, 0x50, 0x4e, 0x47])], "shot.png", { type: "image/png" });

describe("FeedbackPage", () => {
  it("asks for the text before anything is sent", async () => {
    const { api } = renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Send" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Say what happened first.");
    expect(api.sendReport).not.toHaveBeenCalled();
  });

  it("sends the report with a pasted screenshot, then clears the form", async () => {
    const { api, notify } = renderPage();
    await userEvent.type(await screen.findByLabelText(/What happened/), "the radar is empty");
    await userEvent.type(screen.getByLabelText(/Your name/), "Sam");
    fireEvent.paste(screen.getByRole("main"), { clipboardData: { files: [png()] } });
    expect(await screen.findByAltText("Screenshot preview")).toBeInTheDocument();
    await userEvent.click(screen.getByRole("button", { name: "Send" }));
    await vi.waitFor(() => expect(api.sendReport).toHaveBeenCalled());
    expect(api.sendReport).toHaveBeenCalledWith({
      kind: "bug",
      text: "the radar is empty",
      contact: "Sam",
      screenshot: expect.stringMatching(/^[A-Za-z0-9+/=]+$/),
      include_logs: true,
    });
    await vi.waitFor(() => expect(notify).toHaveBeenCalledWith("Thanks, sent", "ok"));
    expect(screen.getByLabelText(/What happened/)).toHaveValue("");
    expect(screen.queryByAltText("Screenshot preview")).toBeNull();
  });

  it("shows the error and keeps the text when sending fails", async () => {
    const api = fakeDesktopApi({ sendReport: vi.fn().mockRejectedValue(new Error("Too many reports, try again in a minute.")) });
    renderPage(api);
    await userEvent.type(await screen.findByLabelText(/What happened/), "x");
    await userEvent.click(screen.getByRole("button", { name: "Send" }));
    expect(await screen.findByRole("alert")).toHaveTextContent("Too many reports");
    expect(screen.getByLabelText(/What happened/)).toHaveValue("x");
    await userEvent.click(screen.getByRole("button", { name: "Save report" }));
    expect(api.saveReport).toHaveBeenCalled();
  });

  it("refuses a screenshot that is not a PNG or JPEG", async () => {
    renderPage();
    await screen.findByRole("button", { name: "Send" });
    fireEvent.paste(screen.getByRole("main"), { clipboardData: { files: [new File(["x"], "a.gif", { type: "image/gif" })] } });
    expect(screen.queryByAltText("Screenshot preview")).toBeNull();
  });

  it("only offers saving when the build cannot send", async () => {
    renderPage(fakeDesktopApi({ reportInfo: vi.fn().mockResolvedValue({ can_send: false }) }));
    expect(await screen.findByText(/Sending is not set up/)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Send" })).toBeDisabled();
  });

  it("says the Windows version goes out with ideas too, and the name only when given", async () => {
    renderPage();
    await userEvent.click(await screen.findByRole("button", { name: "Idea" }));
    const sends = screen.getByText(/What gets sent/);
    expect(sends).toHaveTextContent("the app version and Windows version");
    expect(sends).not.toHaveTextContent("your name");
    await userEvent.type(screen.getByLabelText(/Your name/), "Sam");
    expect(sends).toHaveTextContent("your name");
  });

  it("notes when it could not check whether sending works", async () => {
    renderPage(fakeDesktopApi({ reportInfo: vi.fn().mockRejectedValue(new Error("down")) }));
    expect(await screen.findByText(/Couldn't check whether sending works/)).toBeInTheDocument();
  });
});
