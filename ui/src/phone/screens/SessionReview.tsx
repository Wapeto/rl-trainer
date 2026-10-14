import type { SessionReview as Review } from "@shared/generated/types";
import { Button } from "@shared/components/ui";
import { ReviewSections } from "@shared/review/ReviewSections";

interface SessionReviewProps {
  review: Review;
  now: number;
  subtitle: string;
  onNext: () => void;
}

/** Session end, step 1 of 2. */
export function SessionReviewStep({ review, now, subtitle, onNext }: SessionReviewProps) {
  return (
    <>
      <h1 className="title">Session review</h1>
      <p className="subtitle">{subtitle} · step 1 of 2</p>
      <section className="glass sr-card" aria-label="Coach review">
        <ReviewSections review={review} now={now} layout="phone" />
      </section>
      <div className="save sr-next">
        <Button variant="primary" onClick={onNext}>
          Next: MMR &amp; note
        </Button>
      </div>
    </>
  );
}
