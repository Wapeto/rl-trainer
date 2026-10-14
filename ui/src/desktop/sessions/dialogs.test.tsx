import { render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it, vi } from "vitest";
import type { SessionDetail } from "@shared/generated/types";
import { desktopFixtures as f } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { EditReviewDialog, lastMmr, reviewSizes } from "./EditReviewDialog";
import { SetPackDialog } from "./SetPackDialog";

const unassigned = f.detail.activities[2]; // 84 attempts, no pack

describe("SetPackDialog", () => {
  function open(api = fakeDesktopApi(), suggested: number | null = 11) {
    const onDone = vi.fn();
    const onClose = vi.fn();
    const notify = vi.fn();
    render(
      <SetPackDialog api={api} activity={unassigned} suggestedPackId={suggested} notify={notify} onDone={onDone} onClose={onClose} />,
    );
    return { api, onDone, onClose, notify };
  }

  it("lists visible packs with the one played before first", async () => {
    open();
    const list = await screen.findByRole("listbox", { name: "Packs" });
    const options = within(list).getAllByRole("option");
    expect(options[0]).toHaveTextContent("Aerial Shots - Pass");
    expect(options[0]).toHaveTextContent("Played before this");
    expect(options[0]).toHaveAttribute("aria-selected", "true");
    expect(within(list).queryByText("Consistency")).toBeNull();
    expect(screen.getByText("84 attempts", { exact: false })).toBeInTheDocument();
  });

  it("searches by a code typed without dashes", async () => {
    open();
    await screen.findByRole("listbox", { name: "Packs" });
    await userEvent.type(screen.getByLabelText("Search packs"), "07e181bc");
    await waitFor(() => expect(screen.getAllByRole("option")).toHaveLength(1));
    await waitFor(() => expect(screen.getByRole("option")).toHaveTextContent("Backboard Reads"));
  });

  it("shows the activity's current pack even when it is hidden, selected and tagged", async () => {
    const hiddenPack = f.packList.find((p) => p.pack.hidden)!.pack;
    const activity = { ...unassigned, pack: { id: hiddenPack.id, name: hiddenPack.name, code: hiddenPack.code, target_rate: hiddenPack.target_rate } };
    render(
      <SetPackDialog api={fakeDesktopApi()} activity={activity} suggestedPackId={null} notify={vi.fn()} onDone={vi.fn()} onClose={vi.fn()} />,
    );
    const option = await screen.findByRole("option", { name: new RegExp(hiddenPack.name) });
    expect(option).toHaveAttribute("aria-selected", "true");
    expect(within(option).getByText("Hidden")).toBeInTheDocument();
  });

  it("sets the chosen pack, or clears it", async () => {
    const { api, onDone, onClose } = open();
    await screen.findByRole("listbox", { name: "Packs" });
    await userEvent.click(screen.getByRole("option", { name: /Backboard Reads/ }));
    await userEvent.click(screen.getByRole("button", { name: "Set" }));
    expect(api.setBlockPack).toHaveBeenCalledWith(12, 4);
    expect(onDone).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
    await userEvent.click(screen.getByRole("button", { name: "No pack" }));
    expect(api.setBlockPack).toHaveBeenLastCalledWith(12, null);
  });

  it("stays open when setting the pack fails", async () => {
    const api = fakeDesktopApi({ setBlockPack: vi.fn().mockRejectedValue(new Error("the activity is still being recorded")) });
    const { onClose, notify } = open(api);
    await screen.findByRole("listbox", { name: "Packs" });
    await userEvent.click(screen.getByRole("button", { name: "Set" }));
    expect(onClose).not.toHaveBeenCalled();
    expect(notify).toHaveBeenCalledWith("The activity is still being recorded", "error");
  });
});

describe("EditReviewDialog", () => {
  function open(detail: SessionDetail = f.detail, api = fakeDesktopApi()) {
    const onDone = vi.fn();
    const onClose = vi.fn();
    render(<EditReviewDialog api={api} detail={detail} notify={vi.fn()} onDone={onDone} onClose={onClose} />);
    return { api, onDone, onClose };
  }

  it("offers a field per ranked mode played or already entered", () => {
    expect(reviewSizes(f.detail)).toEqual([2]);
    const withOldEntry = { ...f.detail, mmr: [...f.detail.mmr, { id: 9, team_size: 1, mmr: 640, recorded_at: 1, change: null }] };
    expect(reviewSizes(withOldEntry)).toEqual([1, 2]);
    expect(lastMmr(f.detail, 2)).toBe(912);
    expect(lastMmr(f.detail, 3)).toBeNull();
  });

  it("saves the MMR and the note", async () => {
    const { api, onDone, onClose } = open();
    const field = screen.getByLabelText("2v2 MMR");
    expect(field).toHaveValue("912");
    await userEvent.clear(field);
    await userEvent.type(field, "915");
    await userEvent.clear(screen.getByLabelText("Note"));
    await userEvent.type(screen.getByLabelText("Note"), "Better rotations");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.editReview).toHaveBeenCalledWith(12, [{ team_size: 2, mmr: 915 }], "Better rotations");
    expect(onDone).toHaveBeenCalled();
    expect(onClose).toHaveBeenCalled();
  });

  it("refuses a wrong MMR before sending and removes an emptied one", async () => {
    const { api } = open();
    const field = screen.getByLabelText("2v2 MMR");
    await userEvent.clear(field);
    await userEvent.type(field, "abc");
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(screen.getByText("2v2 MMR must be a whole number from 0 to 3000")).toBeInTheDocument();
    expect(api.editReview).not.toHaveBeenCalled();
    await userEvent.clear(field);
    await userEvent.click(screen.getByRole("button", { name: "Save" }));
    expect(api.editReview).toHaveBeenCalledWith(12, [], f.detail.notes);
  });
});
