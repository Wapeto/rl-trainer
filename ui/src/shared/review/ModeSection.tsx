import type { ModeReview } from "@shared/generated/types";
import { usualGloss } from "../glossary";
import { Hint } from "../hint/Hint";
import { MIN_EARLIER, modeName } from "./format";
import { StatRow } from "./StatRow";

function ModeBody({ mode, name }: { mode: ModeReview; name: string }) {
  if (mode.earlier < MIN_EARLIER) {
    return (
      <p className="rv-note">{`Needs ${MIN_EARLIER} earlier ranked ${name} matches to compare — you have ${mode.earlier}`}</p>
    );
  }
  if (mode.items.length === 0) return <p className="rv-note">Nothing stood out against your usual</p>;
  return (
    <>
      {mode.items.map((i) => (
        <StatRow key={i.key} item={i} />
      ))}
    </>
  );
}

/** One ranked mode against its usual (spec §4.3). */
export function ModeSection({ mode }: { mode: ModeReview }) {
  const name = modeName(mode.team_size);
  return (
    <section className="rv-sec" aria-label={`Ranked ${name}`}>
      <div className="rv-sec__h">
        <Hint gloss={usualGloss(name)}>
          <span className="rv-k">{`Ranked ${name} vs your usual`}</span>
        </Hint>
        <span className="rv-n">{`${mode.matches} ${mode.matches === 1 ? "match" : "matches"}`}</span>
      </div>
      <ModeBody mode={mode} name={name} />
    </section>
  );
}
