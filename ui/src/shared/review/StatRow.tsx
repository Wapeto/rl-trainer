import type { ReviewItem } from "@shared/generated/types";
import { glossFor } from "../glossary";
import { Hint } from "../hint/Hint";
import { coverageNote, describeItem, formatDelta, formatItemValue } from "./format";
import { trackScale } from "./track";

/** One "vs your usual" stat: dot, label, value, usual, range track, difference (spec §4.2). */
export function StatRow({ item }: { item: ReviewItem }) {
  const t = trackScale(item);
  const note = coverageNote(item);
  return (
    <div className={`rv-row rv-row--${item.better ? "good" : "warn"}`} role="group" aria-label={describeItem(item)}>
      <span className="rv-dot" aria-hidden />
      <span className="rv-row__name">
        <Hint gloss={glossFor(item.key)}>{item.label}</Hint>
      </span>
      <b className="rv-row__v">{formatItemValue(item, item.value)}</b>
      <span className="rv-row__u">
        usual {formatItemValue(item, item.usual)}
        {note && ` · ${note}`}
      </span>
      <span className="rv-trk" aria-hidden>
        <i className="rv-trk__band" style={{ left: `${t.bandFrom}%`, width: `${t.bandTo - t.bandFrom}%` }} />
        <i className="rv-trk__usual" style={{ left: `${t.usual}%` }} />
        <i className="rv-trk__tick" style={{ left: `${t.tick}%` }} />
      </span>
      <span className="rv-row__d">{formatDelta(item)}</span>
    </div>
  );
}
