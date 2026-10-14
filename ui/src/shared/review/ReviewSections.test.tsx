import { render, screen, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { describe, expect, it } from "vitest";
import type { SessionReview } from "@shared/generated/types";
import { desktopFixtures as f } from "../../desktop/fixtures";
import { REPLAY_STATS } from "../glossary";
import { isEmptyReview, ReviewSections } from "./ReviewSections";

const NOW = f.review.to_save[0].started_at + 3_600_000;

describe("ReviewSections", () => {
  it("shows a summary, ranked rows against usual, training and replays to save", () => {
    render(<ReviewSections review={f.review} now={NOW} layout="desktop" />);
    expect(screen.getByText("2 things stood out · 1 replay to save")).toBeInTheDocument();
    const row = screen.getByRole("group", { name: "Touches under 20 boost 49%, usual 30%, 19 points above" });
    expect(within(row).getByText("▲ +19 pts")).toBeInTheDocument();
    expect(within(row).getByText("usual 30% · 1 of 3 matches")).toBeInTheDocument();
    expect(screen.getByText("Aerial Shots - Pass")).toBeInTheDocument();
    expect(screen.getByText("20%")).toBeInTheDocument();
    expect(
      screen.getByText("target 65% · Best stretch 8/20 after 7 min · dropped after 20 min · 10 quick resets left out"),
    ).toBeInTheDocument();
    expect(screen.getByText("Needs 5 earlier ranked 1v1 matches to compare — you have 3")).toBeInTheDocument();
    expect(screen.getByText("L 3–8")).toBeInTheDocument();
    expect(screen.getByText("#2 of 20")).toBeInTheDocument();
  });

  it("explains a stat on hover", async () => {
    render(<ReviewSections review={f.review} now={NOW} layout="desktop" />);
    await userEvent.hover(screen.getByText("Touches under 20 boost"));
    expect(await screen.findByRole("tooltip")).toHaveTextContent(REPLAY_STATS.low_boost_touches.text);
  });

  it("says when nothing stands out", () => {
    const review: SessionReview = { ...f.review, modes: [{ team_size: 2, matches: 2, earlier: 20, items: [] }] };
    render(<ReviewSections review={review} now={NOW} layout="phone" />);
    expect(screen.getByText("Nothing stood out against your usual")).toBeInTheDocument();
  });

  it("hides training bars under 3 blocks", () => {
    const review: SessionReview = { ...f.review, training: [{ ...f.review.training[0], bars: [0.5, 0.6] }] };
    const { container } = render(<ReviewSections review={review} now={NOW} layout="desktop" />);
    expect(container.querySelector(".rv-bars")).toBeNull();
  });

  it("shows 3 replays to save, then the rest on demand", async () => {
    const to_save = Array.from({ length: 5 }, (_, i) => ({ ...f.review.to_save[0], match_id: 100 + i, place: 15 + i }));
    render(<ReviewSections review={{ ...f.review, to_save }} now={NOW} layout="desktop" />);
    expect(screen.getAllByText(/^#\d+ of 20$/)).toHaveLength(3);
    await userEvent.click(screen.getByRole("button", { name: "+ 2 more" }));
    expect(screen.getAllByText(/^#\d+ of 20$/)).toHaveLength(5);
    expect(screen.getByText("#17 of 20")).toHaveClass("rv-pill--soon");
    await userEvent.click(screen.getByRole("button", { name: "Show less" }));
    expect(screen.getAllByText(/^#\d+ of 20$/)).toHaveLength(3);
  });

  it("cuts a long pack name short and keeps it whole on hover", () => {
    const name = "Ultimate Ceiling Shot Double Touch Redirect Practice Pack";
    const review: SessionReview = { ...f.review, training: [{ ...f.review.training[0], pack_name: name }] };
    render(<ReviewSections review={review} now={NOW} layout="phone" />);
    const el = screen.getByText(name);
    expect(el).toHaveClass("rv-tr__name");
    expect(el).toHaveAttribute("title", name);
  });

  it("knows an empty review", () => {
    expect(isEmptyReview({ ...f.review, training: [], modes: [], to_save: [] })).toBe(true);
    expect(isEmptyReview(f.review)).toBe(false);
  });
});
