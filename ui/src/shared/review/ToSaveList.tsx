import { useState } from "react";
import type { ReplayToSave } from "@shared/generated/types";
import { placeGloss, TERMS } from "../glossary";
import { Hint } from "../hint/Hint";
import { toSaveParts } from "./format";
import { HISTORY_SIZE, SOON_GONE_PLACE } from "./history";

const SHOWN = 3;

/** This session's replays still to save, most urgent first (spec §4.5). */
export function ToSaveList({ items }: { items: ReplayToSave[] }) {
  const [all, setAll] = useState(false);
  const shown = all ? items : items.slice(0, SHOWN);
  const hidden = items.length - SHOWN;
  return (
    <section className="rv-sec" aria-label="Replays to save">
      <div className="rv-sec__h">
        <Hint gloss={TERMS.to_save}>
          <span className="rv-k">Replays to save · this session</span>
        </Hint>
      </div>
      <p className="rv-m">Save them from Match History in game</p>
      {shown.map((m) => {
        const p = toSaveParts(m);
        return (
          <div key={m.match_id} className="rv-sv">
            <b className={`rv-sv__r rv-sv__r--${p.tone}`}>{p.result}</b>
            <span className="rv-n">{p.mode}</span>
            <span className="rv-sv__t">{p.time}</span>
            <Hint gloss={placeGloss(m.place)}>
              <span className={`rv-pill${m.place >= SOON_GONE_PLACE ? " rv-pill--soon" : ""}`}>
                {`#${m.place} of ${HISTORY_SIZE}`}
              </span>
            </Hint>
          </div>
        );
      })}
      {hidden > 0 && (
        <button type="button" className="rv-more" onClick={() => setAll(!all)}>
          {all ? "Show less" : `+ ${hidden} more`}
        </button>
      )}
    </section>
  );
}
