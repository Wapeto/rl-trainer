import { timeOfDay } from "@shared/format";
import { isEmptyReview, ReviewSections } from "@shared/review/ReviewSections";
import type { DesktopApi } from "../api";
import { useLoad } from "../components/useLoad";

interface CoachReviewProps {
  api: DesktopApi;
  sessionId: number;
  reload: string;
}

/** The after-session review under the session's KPIs (spec §5.3). */
export function CoachReview({ api, sessionId, reload }: CoachReviewProps) {
  const loaded = useLoad(() => api.sessionReview(sessionId), `${sessionId}|${reload}`);
  const review = loaded.data;
  if (!review || review.session_id !== sessionId || isEmptyReview(review)) return null;
  return (
    <section className="d-coach glass" aria-label="Coach review">
      <div className="d-coach__top">
        <b>Coach review</b>
        {review.replay_added_at != null && (
          <span className="d-k">updated {timeOfDay(review.replay_added_at)} · replay added</span>
        )}
      </div>
      <ReviewSections review={review} now={Date.now()} layout="desktop" />
    </section>
  );
}
