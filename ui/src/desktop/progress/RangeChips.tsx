import type { ProgressRange } from "@shared/generated/types";
import { RANGE_LABEL } from "./format";

const RANGES: ProgressRange[] = ["days30", "days90", "all"];

export function RangeChips({ range, onRange }: { range: ProgressRange; onRange: (r: ProgressRange) => void }) {
  return (
    <div className="p-seg glass" role="radiogroup" aria-label="Range">
      {RANGES.map((r) => (
        <button key={r} type="button" role="radio" aria-checked={r === range} className={r === range ? "p-seg--on" : ""} onClick={() => onRange(r)}>
          {RANGE_LABEL[r]}
        </button>
      ))}
    </div>
  );
}
