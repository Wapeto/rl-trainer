import { useEffect, useRef, useState } from "react";
import { Button, Pill } from "@shared/components/ui";
import { dayLabel, longDuration, timeOfDay } from "@shared/format";
import type { MmrInput, SessionReview as Review, SessionSummary } from "@shared/generated/types";
import { isEmptyReview } from "@shared/review/ReviewSections";
import { act } from "../act";
import { reviewTarget } from "../selectScreen";
import { Header } from "../Header";
import { MMR_MAX, MmrWheel } from "../MmrWheel";
import type { ScreenProps } from "../screenProps";
import { SessionReviewStep } from "./SessionReview";
import { SessionStats } from "./SessionStats";

export function isGoodSession(s: SessionSummary): boolean {
  if (s.wins > s.losses) return true;
  return (
    s.training_make_rate != null && s.training_target_rate != null && s.training_make_rate >= s.training_target_rate
  );
}

/** The session's review, or "none" when it is empty or cannot load (step 1 is then skipped). */
function useReview(api: ScreenProps["api"], sessionId: number): Review | null | "none" {
  const [review, setReview] = useState<Review | null | "none">(null);
  useEffect(() => {
    let live = true;
    if (!api) {
      setReview("none");
      return;
    }
    api.sessionReview(sessionId).then(
      (r) => live && setReview(r && !isEmptyReview(r) ? r : "none"),
      () => live && setReview("none"),
    );
    return () => {
      live = false;
    };
  }, [api, sessionId]);
  return review;
}

/** Session end in two steps (spec §5.2): the review, then MMR and the note. */
export function SessionEnd({ snap, conn, now, api, notify }: ScreenProps) {
  const s = reviewTarget(snap)!;
  const inMenus = snap.live.session != null;
  const lastMmr = (size: number) => snap.live.latest_mmr.find((m) => m.team_size === size)?.mmr ?? null;
  const [values, setValues] = useState<Record<number, number | null>>(() =>
    Object.fromEntries(s.ranked_team_sizes.map((size) => [size, lastMmr(size)])),
  );
  const [notes, setNotes] = useState(s.notes);
  const [busy, setBusy] = useState(false);
  const [step, setStep] = useState<1 | 2>(1);
  const sending = useRef(false);
  const review = useReview(api, s.id);
  const hasReview = review !== null && review !== "none";
  const good = isGoodSession(s);
  const when = `${dayLabel(s.started_at, now)} ${timeOfDay(s.started_at)} – ${timeOfDay(s.ended_at)}`;

  async function submit(skip: boolean) {
    if (!api || sending.current) return;
    const mmr: MmrInput[] = [];
    if (!skip) {
      for (const size of s.ranked_team_sizes) {
        const n = values[size];
        if (n == null) continue;
        if (!Number.isInteger(n) || n < 0 || n > MMR_MAX) {
          notify(`MMR ${size}v${size} must be a whole number from 0 to ${MMR_MAX}`, "error");
          return;
        }
        mmr.push({ team_size: size, mmr: n });
      }
    }
    sending.current = true;
    setBusy(true);
    try {
      await act(notify, () => api.reviewSession(s.id, mmr, skip ? null : notes), skip ? "Skipped" : "Saved");
    } finally {
      sending.current = false;
      setBusy(false);
    }
  }

  const header = (
    <Header
      conn={conn}
      left={inMenus ? <Pill live>In the menus</Pill> : <Pill off>Session ended</Pill>}
      right={<Pill>{longDuration(s.ended_at - s.started_at)}</Pill>}
    />
  );
  if (review === null) {
    return (
      <div className="screen">
        {header}
        <p className="subtitle">Loading…</p>
      </div>
    );
  }
  if (hasReview && step === 1) {
    return (
      <div className="screen">
        {header}
        <SessionReviewStep review={review} now={now} subtitle={when} onNext={() => setStep(2)} />
      </div>
    );
  }
  return (
    <div className={`screen ${good ? "tone-green" : ""}`.trim()}>
      {header}
      <h1 className="title">{good ? "Good session" : "Session summary"}</h1>
      <p className="subtitle">
        {when}
        {hasReview && " · step 2 of 2"}
      </p>
      <SessionStats s={s} />
      {s.ranked_team_sizes.map((size) => {
        const last = lastMmr(size);
        return (
          <div key={size} className="field glass field--wheel">
            <span className="field__l">
              MMR {size}v{size}
              <small>{last == null ? "first entry" : `last ${last}`}</small>
            </span>
            <MmrWheel
              label={`MMR ${size}v${size}`}
              value={values[size] ?? null}
              last={last}
              onChange={(v) => setValues((prev) => ({ ...prev, [size]: v }))}
            />
          </div>
        );
      })}
      <textarea
        className="note glass"
        aria-label="Note"
        placeholder="Add a note… (optional)"
        maxLength={2000}
        value={notes}
        onChange={(e) => setNotes(e.target.value)}
      />
      <div className="save">
        <Button variant="primary" disabled={busy || !api} onClick={() => submit(false)}>
          Save
        </Button>
        <Button variant="text" disabled={busy || !api} onClick={() => submit(true)}>
          Skip
        </Button>
        {hasReview && (
          <Button variant="text" onClick={() => setStep(1)}>
            Back to review
          </Button>
        )}
      </div>
    </div>
  );
}
