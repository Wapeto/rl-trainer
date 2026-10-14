import type { ModeProgress, ModeStat, ProgressRange, StatSeries } from "@shared/generated/types";
import { useState } from "react";
import { PROGRESS, statGloss } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { ChartCard } from "../charts/ChartCard";
import type { ChartPoint } from "../charts/LineChart";
import { readKey, writeKey } from "../components/storage";
import { useLoad } from "../components/useLoad";
import { plural } from "../format";
import { changeTone, MODE_COLORS, modeName, pointsChange, shortDate, signed, winPct } from "./format";
import type { ProgressPageProps } from "./ProgressPage";
import { formatStat, GROUPS, STATS, type StatGroup } from "./statMeta";
import { WinnersCard } from "./WinnersCard";

const TAB_KEY = "rl-progress-tab";
/** The stat shown large in each tab: `rl-progress-focus-<tab>`. */
const FOCUS_KEY = "rl-progress-focus-";

type ModeViewProps = ProgressPageProps & { range: ProgressRange; teamSize: number };

function statPoints(s: StatSeries): ChartPoint[] {
  return s.points.map((p) => ({
    at: p.at,
    value: p.value,
    sessionId: p.session_id,
    tip: [formatStat(s.stat, p.value), `${shortDate(p.at)} · ${p.wins}W ${p.losses}L`, ...(p.partial ? ["partial"] : [])],
  }));
}

function cardValue(s: StatSeries): string {
  const m = STATS[s.stat];
  return `${formatStat(s.stat, s.overall)}${m.perGame ? " / game" : ""}`;
}

function since(stats: StatSeries[]): string | null {
  const dated = stats.filter((s) => s.since != null);
  if (dated.length === 0) return null;
  const latest = dated.reduce((a, b) => ((b.since ?? 0) > (a.since ?? 0) ? b : a));
  return `since ${shortDate(latest.since ?? 0)} · ${plural(latest.matches, "match", "matches")}`;
}

function readTab(storage: ModeViewProps["storage"]): StatGroup {
  const saved = readKey(storage, TAB_KEY);
  return GROUPS.find((g) => g.id === saved)?.id ?? "scoreboard";
}

function statChart(s: StatSeries, onOpenSession: (id: number) => void) {
  const meta = STATS[s.stat];
  return {
    title: meta.label,
    value: cardValue(s),
    gloss: statGloss(s.stat, meta.label),
    note: meta.estimated ? "estimated" : null,
    chart: { series: [{ label: meta.label, color: "#7aa7ff", points: statPoints(s) }], unit: meta.unit === "%" ? "%" : "", onOpenSession },
  };
}

export function ModeView({ api, range, reload, teamSize, storage, onOpenSession }: ModeViewProps) {
  const loaded = useLoad(() => api.progressMode(teamSize, range), `${teamSize}|${range}|${reload}`);
  const [tab, setTab] = useState<StatGroup>(() => readTab(storage));
  const [chosen, setChosen] = useState<Partial<Record<StatGroup, string>>>({});
  const p: ModeProgress | undefined = loaded.data;
  if (!p) return <p className="d-empty">{loaded.error ?? "Loading…"}</p>;
  const stat = (id: string) => p.stats.find((s) => s.stat === id);
  const shooting = stat("shooting_pct");
  const winRate = stat("win_rate");
  const groupStats = p.stats.filter((s) => STATS[s.stat].group === tab);
  const color = MODE_COLORS[teamSize];

  const saved = chosen[tab] ?? readKey(storage, FOCUS_KEY + tab);
  const focused = groupStats.find((s) => s.stat === saved) ?? groupStats[0];

  function focus(stat: ModeStat) {
    writeKey(storage, FOCUS_KEY + tab, stat);
    setChosen({ ...chosen, [tab]: stat });
  }

  function choose(next: StatGroup) {
    setTab(next);
    writeKey(storage, TAB_KEY, next);
  }

  return (
    <>
      <div className="d-hd">
        <div>
          <h1>{`Ranked ${modeName(teamSize)}`}</h1>
          <p className="p-sub">
            {plural(p.matches, "match", "matches")} in {plural(p.sessions, "session")}
          </p>
        </div>
      </div>
      <div className="d-kpis">
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.mmr}>MMR</Hint>
          </div>
          <div className="d-kpi__v">
            {p.mmr_latest ?? "—"}
            <small className={changeTone(p.mmr_change)}>{signed(p.mmr_change)}</small>
          </div>
          <div className="p-sub">
            from {p.mmr_first ?? "—"} · high {p.mmr_high ?? "—"}
          </div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.record}>Record</Hint>
          </div>
          <div className="d-kpi__v">
            <span className="d-g">{p.wins}W</span> <span className="d-r">{p.losses}L</span>
          </div>
          <div className="p-sub">
            {winPct(p.wins, p.losses) ?? "—"}% won · {p.overtimes} OT
          </div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.shooting}>Shooting</Hint>
          </div>
          <div className="d-kpi__v">
            {formatStat("shooting_pct", shooting?.overall)}
            <small className={changeTone(shooting?.change)}>{shooting?.change == null ? "" : pointsChange(shooting.change / 100)}</small>
          </div>
          <div className="p-sub">
            {formatStat("goals", stat("goals")?.overall)} goals of {formatStat("shots", stat("shots")?.overall)} shots per game
          </div>
        </div>
        <div className="d-kpi glass">
          <div className="d-k">
            <Hint gloss={PROGRESS.team_share}>Team share</Hint>
          </div>
          <div className="d-kpi__v">
            {formatStat("share_score", stat("share_score")?.overall)}
            <small>of score</small>
          </div>
          <div className="p-sub">possession {formatStat("possession", stat("possession")?.overall)}</div>
        </div>
      </div>
      <div className="p-grid2">
        <ChartCard
          title="MMR"
          value={String(p.mmr_latest ?? "—")}
          gloss={PROGRESS.mmr_chart}
          compact={false}
          height={130}
          chart={{
            series: [{ label: "MMR", color, points: p.mmr.map((m) => ({ at: m.at, value: m.mmr, sessionId: m.session_id, tip: [String(m.mmr), shortDate(m.at)] })) }],
            onOpenSession,
          }}
        />
        {winRate && (
          <ChartCard
            title="Win rate"
            value={formatStat("win_rate", winRate.overall)}
            gloss={statGloss("win_rate", "Win rate")}
            compact={false}
            height={130}
            chart={{ series: [{ label: "Win rate", color, points: statPoints(winRate) }], reference: 50, unit: "%", onOpenSession }}
          />
        )}
      </div>
      <WinnersCard api={api} teamSize={teamSize} />
      <div className="p-tabs" role="tablist" aria-label="Stat groups">
        {GROUPS.map((g) => (
          <button key={g.id} type="button" role="tab" aria-selected={g.id === tab} onClick={() => choose(g.id)}>
            {g.label}
          </button>
        ))}
        {since(groupStats) && <span className="p-since">{since(groupStats)}</span>}
      </div>
      <div role="tabpanel" aria-label={GROUPS.find((g) => g.id === tab)?.label}>
        {focused && (
          <section className="p-focus" aria-label="Selected stat">
            <ChartCard {...statChart(focused, onOpenSession)} compact={false} height={210} />
          </section>
        )}
        <div className="p-grid4">
          {groupStats.map((s) => (
            <ChartCard
              key={s.stat}
              {...statChart(s, onOpenSession)}
              height={56}
              selected={s.stat === focused?.stat}
              onSelect={() => focus(s.stat)}
            />
          ))}
        </div>
      </div>
    </>
  );
}
