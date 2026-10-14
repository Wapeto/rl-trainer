import type { SessionReview } from "@shared/generated/types";
import { MIN_EARLIER, summaryLine } from "./format";
import { ModeSection } from "./ModeSection";
import { ToSaveList } from "./ToSaveList";
import { TrainingSection } from "./TrainingSection";
import "./review.css";

export function isEmptyReview(r: SessionReview): boolean {
  return r.training.length === 0 && r.modes.length === 0 && r.to_save.length === 0;
}

interface ReviewSectionsProps {
  review: SessionReview;
  now: number;
  layout: "phone" | "desktop";
}

/**
 * The review, shared by the phone (step 1) and the desktop session page (spec §4.6): the summary,
 * the modes with stat rows full width, then training and short mode notes beside the replays to save.
 */
export function ReviewSections({ review, layout }: ReviewSectionsProps) {
  const summary = summaryLine(review);
  const rows = review.modes.filter((m) => m.earlier >= MIN_EARLIER && m.items.length > 0);
  const notes = review.modes.filter((m) => !rows.includes(m));
  const left = review.training.length > 0 || notes.length > 0;
  return (
    <div className={`rv rv--${layout}`}>
      {summary && <p className="rv-sum">{summary}</p>}
      {rows.map((m) => (
        <ModeSection key={m.team_size} mode={m} />
      ))}
      {(left || review.to_save.length > 0) && (
        <div className="rv-cols">
          {left && (
            <div className="rv-col">
              {review.training.length > 0 && <TrainingSection items={review.training} />}
              {notes.map((m) => (
                <ModeSection key={m.team_size} mode={m} />
              ))}
            </div>
          )}
          {review.to_save.length > 0 && (
            <div className="rv-col">
              <ToSaveList items={review.to_save} />
            </div>
          )}
        </div>
      )}
    </div>
  );
}
