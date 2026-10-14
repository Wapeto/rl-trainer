import { Glass, Kv, Pill, StatRow } from "@shared/components/ui";
import { elapsed, kmh, whole } from "@shared/format";
import type { LiveFreeplay } from "@shared/generated/types";
import { Header, sessionPill } from "../Header";
import type { ScreenProps } from "../screenProps";

const EMPTY: LiveFreeplay = {
  touches: 0,
  hard_hits: 0,
  max_hit_speed: 0,
  goals: 0,
  pct_supersonic: 0,
  pct_on_wall: 0,
  pct_airborne: 0,
  best_today_hit: 0,
};

export function Freeplay({ snap, conn, now }: ScreenProps) {
  const block = snap.live.block!;
  const f = block.freeplay ?? EMPTY;
  return (
    <div className="screen tone-gold">
      <Header conn={conn} left={<Pill live>Free play</Pill>} right={sessionPill(snap, now)} />
      <h1 className="title">Free play</h1>
      <p className="subtitle">Free play {elapsed(now - block.started_at)}</p>
      <div className="hero hero--number">
        <div className="hero__number">
          {kmh(f.max_hit_speed)}
          <small>km/h</small>
        </div>
        <div className="hero__label">Hardest hit</div>
        <span className="chip glass">Best today {kmh(f.best_today_hit)} km/h</span>
      </div>
      <StatRow
        items={[
          { label: "Touches", value: f.touches },
          { label: "Hard hits", value: f.hard_hits },
          { label: "Goals", value: f.goals },
        ]}
      />
      <Glass className="kv-card">
        <Kv label="Supersonic" value={`${whole(f.pct_supersonic)}%`} />
        <Kv label="On the wall" value={`${whole(f.pct_on_wall)}%`} />
        <Kv label="In the air" value={`${whole(f.pct_airborne)}%`} />
      </Glass>
    </div>
  );
}
