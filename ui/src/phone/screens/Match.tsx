import { Ring, type Tone } from "@shared/components/Ring";
import { Glass, Pill } from "@shared/components/ui";
import { arenaName, clock, kmh, modeLabel, whole } from "@shared/format";
import type { CompareStat, StatCompare } from "@shared/generated/types";
import { Header } from "../Header";
import type { ScreenProps } from "../screenProps";

/** The "Hardest shot" ring is full at this speed. */
export const HARDEST_SHOT_SCALE_KMH = 150;

const STAT_LABEL: Record<CompareStat, string> = {
  score: "Score",
  goals: "Goals",
  assists: "Assists",
  saves: "Saves",
  shots: "Shots",
};

export function bestText(row: StatCompare): string {
  if (row.mine_is_best) return "best ✓";
  if (row.best === 0) return "—";
  if (row.tie) return `${row.best} · tie`;
  return row.best_name ? `${row.best} · ${row.best_name}` : String(row.best);
}

interface TileProps {
  label: string;
  tone: Tone;
  fill: number;
  value: string;
  suffix?: string;
  unit?: string;
}

function Tile({ label, tone, fill, value, suffix, unit }: TileProps) {
  return (
    <Glass className="tile">
      <div className="tile__label">{label}</div>
      <Ring size="tile" tone={tone} value={fill} label={`${label} ${value}${suffix ?? (unit ? ` ${unit.toLowerCase()}` : "")}`}>
        <div className="tile__num">
          {value}
          {suffix && <small>{suffix}</small>}
        </div>
        {unit && <div className="tile__unit">{unit}</div>}
      </Ring>
    </Glass>
  );
}

export function Match({ snap, conn }: ScreenProps) {
  const m = snap.live.block!.match;
  if (!m) {
    return (
      <div className="screen">
        <Header conn={conn} left={<Pill live>Match</Pill>} right={null} />
        <h1 className="title">Match</h1>
        <p className="subtitle">Waiting for match data</p>
      </div>
    );
  }
  return (
    <div className="screen">
      <Header
        conn={conn}
        left={<Pill live>{modeLabel(m)}</Pill>}
        right={<Pill>{`Today W${m.today_wins} · L${m.today_losses}`}</Pill>}
      />
      <h1 className="title">{arenaName(m.arena)}</h1>
      <p className="subtitle">{m.overtime ? `+${clock(m.clock_seconds)} OT` : `${clock(m.clock_seconds)} left`}</p>
      <div className="score" aria-label={`Score ${m.team_score} to ${m.opponent_score}`}>
        <div className="score__team score__team--blue">
          <span className="score__n">{m.team_score}</span>YOU
        </div>
        <span className="score__dash" aria-hidden>
          –
        </span>
        <div className="score__team score__team--orange">
          <span className="score__n">{m.opponent_score}</span>THEM
        </div>
      </div>
      <Glass className="cmp">
        <div className="cmp__head">
          <span>Stat</span>
          <span>You</span>
          <span>Best</span>
        </div>
        {m.compare.map((row) => (
          <div key={row.stat} className={`cmp__row${row.mine_is_best ? " cmp__row--best" : ""}`}>
            <span>{STAT_LABEL[row.stat]}</span>
            <b>{row.mine}</b>
            <span className="cmp__best" title={bestText(row)}>
              {bestText(row)}
            </span>
          </div>
        ))}
      </Glass>
      <div className="tiles">
        <Tile label="Hardest shot" tone="blue" fill={m.hardest_hit / HARDEST_SHOT_SCALE_KMH} value={kmh(m.hardest_hit)} unit="KM/H" />
        <Tile label="Supersonic" tone="blue" fill={m.pct_supersonic / 100} value={whole(m.pct_supersonic)} suffix="%" />
        <Tile label="Avg boost" tone="gold" fill={m.avg_boost / 100} value={whole(m.avg_boost)} />
        <Tile label="Time at 0 boost" tone="red" fill={m.pct_zero_boost / 100} value={whole(m.pct_zero_boost)} suffix="%" />
      </div>
    </div>
  );
}
