import { render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { timeOfDay } from "@shared/format";
import { desktopFixtures as f } from "../fixtures";
import { fakeDesktopApi } from "../test/fakeApi";
import { CoachReview } from "./CoachReview";

describe("CoachReview", () => {
  it("shows the review rows with the replay update time", async () => {
    render(<CoachReview api={fakeDesktopApi()} sessionId={12} reload="r" />);
    expect(await screen.findByRole("region", { name: "Coach review" })).toBeInTheDocument();
    expect(screen.getByRole("group", { name: /^Touches under 20 boost 49%, usual 30%/ })).toBeInTheDocument();
    expect(screen.getByText(`updated ${timeOfDay(f.review.replay_added_at!)} · replay added`)).toBeInTheDocument();
  });

  it("shows nothing for an empty review or none", async () => {
    const empty = { ...f.review, training: [], modes: [], to_save: [] };
    const { container, rerender } = render(
      <CoachReview
        api={fakeDesktopApi({
          sessionReview: vi.fn().mockResolvedValue(empty),
        })}
        sessionId={12}
        reload="a"
      />,
    );
    await new Promise((r) => setTimeout(r, 0));
    expect(container).toBeEmptyDOMElement();
    rerender(
      <CoachReview
        api={fakeDesktopApi({ sessionReview: vi.fn().mockResolvedValue(null) })}
        sessionId={12}
        reload="b"
      />,
    );
    await new Promise((r) => setTimeout(r, 0));
    expect(container).toBeEmptyDOMElement();
  });
});
