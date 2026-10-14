import type { RecentAttempt } from "@shared/generated/types";

const SLOTS = 10;

function dotClass(attempt: RecentAttempt | null, newest: boolean): string {
  if (!attempt) return "dot dot--empty";
  const kind = attempt.skipped ? "skip" : attempt.outcome === "make" ? "make" : "miss";
  return `dot dot--${kind}${newest ? " dot--newest" : ""}`;
}

/** The last 10 attempts, oldest first, newest on the right. */
export function OutcomeDots({ attempts }: { attempts: RecentAttempt[] }) {
  const recent = attempts.slice(-SLOTS);
  const slots: (RecentAttempt | null)[] = [
    ...Array<null>(SLOTS - recent.length).fill(null),
    ...recent,
  ];
  return (
    <div className="dots" aria-label="Last 10 attempts">
      {slots.map((attempt, i) => (
        <span
          key={attempt ? attempt.id : `empty-${i}`}
          className={dotClass(attempt, attempt !== null && i === SLOTS - 1)}
          title={attempt ? `#${attempt.seq} ${attempt.skipped ? "skipped" : attempt.outcome}` : undefined}
        />
      ))}
    </div>
  );
}
