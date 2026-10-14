import type { MatchSummary } from "@shared/generated/types";
import { noReplayGloss, TERMS } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { inHistory, SOON_GONE_PLACE } from "@shared/review/history";

interface ReplayCellProps {
  match: MatchSummary;
  startedAt: number;
  now: number;
  onOpen: () => void;
}

export function ReplayCell({ match, startedAt, now, onOpen }: ReplayCellProps) {
  if (match.replay) {
    return (
      <Hint gloss={TERMS.replay_stats} control>
        <button type="button" className="d-chip d-chip--ok" onClick={onOpen}>
          Replay
        </button>
      </Hint>
    );
  }
  if (!inHistory(match.history_place, startedAt, now)) return null;
  const soon = match.history_place >= SOON_GONE_PLACE;
  return (
    <Hint gloss={noReplayGloss(match.history_place)} control>
      <span className={`d-chip d-chip--save${soon ? " d-chip--soon" : ""}`} tabIndex={0}>
        {soon ? "Save soon" : "Save"}
      </span>
    </Hint>
  );
}
