import type { TrainingItem } from "@shared/generated/types";
import { TERMS } from "../glossary";
import { Hint } from "../hint/Hint";
import { trainingBelow, trainingDetail, trainingRate } from "./format";

/** Bars need 3 blocks of 10 attempts to show a trend; fewer would draw one flat block. */
const MIN_BARS = 3;

export function TrainingSection({ items }: { items: TrainingItem[] }) {
  return (
    <section className="rv-sec" aria-label="Training">
      <div className="rv-sec__h">
        <Hint gloss={TERMS.training}>
          <span className="rv-k">Training</span>
        </Hint>
      </div>
      {items.map((t) => {
        const rate = trainingRate(t);
        const name = t.pack_name ?? "No pack";
        return (
          <div key={t.pack_id ?? "none"} className={`rv-tr rv-tr--${trainingBelow(t) ? "warn" : "good"}`}>
            <div className="rv-tr__line">
              <span className="rv-tr__name" title={name}>
                {name}
              </span>
              {rate && (
                <Hint gloss={TERMS.make_rate}>
                  <b>{rate}</b>
                </Hint>
              )}
            </div>
            <Hint gloss={t.best_makes != null ? TERMS.best_stretch : null}>
              <span className="rv-m">{trainingDetail(t)}</span>
            </Hint>
            {t.bars.length >= MIN_BARS && (
              <span className="rv-bars" aria-hidden>
                {t.bars.map((b, i) => (
                  <i key={i} style={{ height: `${Math.max(6, Math.round(b * 100))}%` }} />
                ))}
              </span>
            )}
          </div>
        );
      })}
    </section>
  );
}
