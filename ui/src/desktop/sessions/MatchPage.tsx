import { ArrowLeft } from "lucide-react";
import { type ReactNode, useEffect, useRef, useState } from "react";
import { arenaName, modeLabel, timeOfDay } from "@shared/format";
import type { ActivityRow, Benchmark, Insight, PlayerReplayStats, ReplayPlayer } from "@shared/generated/types";
import { Hint } from "@shared/hint/Hint";
import type { DesktopApi } from "../api";
import { useLoad } from "../components/useLoad";
import { longDay } from "../format";
import { GroupRadar, OverviewRadar } from "./RadarBlocks";
import { resultText } from "./SessionTimeline";
import { direction } from "./benchRadar";
import { KEY_STATS, MIN_POOL_MATCHES, STAT_GROUPS, type StatGroup, bestValue, fromMine, statValue } from "./replayRows";

interface MatchPageProps {
  api: DesktopApi;
  activity: ActivityRow;
  onBack: () => void;
}

/** One column of the tables: a player, or the winners' means (dim, no bar, never bold). */
interface Col {
  id: string;
  name: string;
  bot: boolean;
  me: boolean;
  bench: boolean;
  side: "us" | "them";
  stats: PlayerReplayStats;
}

/** Me, the winners, my teammates, then the opponents (by team relative to me). */
function columns(players: ReplayPlayer[], winners: PlayerReplayStats | null): Col[] {
  const mine = players.find((p) => p.is_me)?.team ?? 0;
  const rank = (p: ReplayPlayer) => (p.team === mine ? 0 : 2) + (p.is_me ? 0 : 1);
  return [...players]
    .sort((a, b) => rank(a) - rank(b) || a.slot - b.slot)
    .flatMap((p) => {
      const col: Col = { id: `p${p.slot}`, name: p.name, bot: p.is_bot, me: p.is_me, bench: false, side: p.team === mine ? "us" : "them", stats: p.stats };
      return p.is_me && winners ? [col, { id: "winners", name: "Winners", bot: false, me: false, bench: true, side: "us" as const, stats: winners }] : [col];
    });
}

const FLASH_MS = 2000;

/** Every player's replay stats for one match, with what stood out (match insights spec §4). */
export function MatchPage({ api, activity, onBack }: MatchPageProps) {
  const m = activity.match_summary!;
  const loaded = useLoad(() => api.matchReplay(m.match_id), String(m.match_id));
  const data = loaded.data;
  const fallback = m.replay != null && (data?.players.length === 0 || loaded.error != null);
  const players = fallback ? [{ slot: 0, team: 0, name: "You", is_me: true, is_bot: false, stats: fromMine(m.replay!) }] : (data?.players ?? []);
  const bench = fallback ? null : (data?.benchmark ?? null);
  const cols = columns(players, bench?.winners ?? null);
  const n = m.team_size ?? 1;
  const mode = `${n}v${n}`;
  const team = n > 1;
  const groups = [KEY_STATS, ...STAT_GROUPS].filter((gr) => team || !gr.teamOnly);
  const [open, setOpen] = useState<Set<string>>(() => new Set([KEY_STATS.name]));
  const [current, setCurrent] = useState(KEY_STATS.name);
  const [flash, setFlash] = useState<string | null>(null);
  const top = useRef<HTMLElement>(null);

  useEffect(() => {
    if (!flash) return;
    const id = setTimeout(() => setFlash(null), FLASH_MS);
    return () => clearTimeout(id);
  }, [flash]);

  // The index marks the last group whose card has scrolled under it.
  useEffect(() => {
    const main = top.current?.closest(".d-main");
    if (!main) return;
    const spy = () => {
      const passed = groups.filter((gr) => (document.getElementById(`ps-${gr.name}`)?.getBoundingClientRect().top ?? 1) <= 80);
      if (passed.length > 0) setCurrent(passed[passed.length - 1].name);
    };
    main.addEventListener("scroll", spy);
    return () => main.removeEventListener("scroll", spy);
  }, [groups]);

  function show(group: string, rowKey?: string) {
    setOpen((o) => new Set(o).add(group));
    setCurrent(group);
    if (rowKey) setFlash(rowKey);
    const target = rowKey ? `ps-row-${group}-${rowKey}` : `ps-${group}`;
    requestAnimationFrame(() => document.getElementById(target)?.scrollIntoView?.({ block: rowKey ? "center" : "start", behavior: "smooth" }));
  }

  const tone = m.result === "win" ? "d-g" : m.result === "loss" ? "d-r" : "d-m";
  return (
    <section className="d-mp" ref={top}>
      <button type="button" className="d-btn" onClick={onBack}>
        <ArrowLeft className="d-icon" aria-hidden />
        Back
      </button>
      <div className="d-hd">
        <div>
          <h1 className={tone}>
            {resultText(m.result, m.team_score, m.opponent_score)}
            {m.overtime && " · OT"}
          </h1>
          <div className="d-meta">
            {modeLabel({ ...m, playlist_id: activity.playlist_id })} · {arenaName(m.arena)} · {longDay(activity.started_at)},{" "}
            {timeOfDay(activity.started_at)}
          </div>
        </div>
      </div>
      {loaded.error && <p className="d-err">{loaded.error}</p>}
      {data === undefined && !loaded.error && <p className="d-empty">Loading…</p>}
      {(data || fallback) && (
        <Summary
          insights={fallback ? null : data!.insights}
          note={bench && bench.winners == null ? `The winners' benchmark appears after ${MIN_POOL_MATCHES} ranked replays in ${mode} (${bench.matches} so far).` : null}
          radar={bench?.winners ? <OverviewRadar bench={bench} team={team} mode={mode} /> : null}
          onPick={(i) => show(KEY_STATS.name, i.stat)}
        />
      )}
      {cols.length > 0 && (
        <>
          <nav className="d-idx" aria-label="Categories">
            {groups.map((gr) => (
              <button key={gr.name} type="button" className="d-idx__c" aria-current={gr.name === current || undefined} onClick={() => show(gr.name)}>
                {gr.name}
              </button>
            ))}
          </nav>
          {groups.map((gr) =>
            open.has(gr.name) ? (
              <GroupTable key={gr.name} group={gr} cols={cols} team={team} flash={flash} bench={bench?.winners ? bench : null} />
            ) : (
              <button key={gr.name} id={`ps-${gr.name}`} type="button" className="d-ps__fold glass" onClick={() => show(gr.name)}>
                {gr.name}
                <span>Show ▾</span>
              </button>
            ),
          )}
        </>
      )}
    </section>
  );
}

function Summary({ insights, note, radar, onPick }: { insights: Insight[] | null; note: string | null; radar: ReactNode; onPick: (i: Insight) => void }) {
  return (
    <section className="d-ins glass" aria-label="What stood out">
      <div className="d-ins__row">
        <div className="d-ins__list">
          <div className="d-k">What stood out</div>
          {insights == null && <p className="d-ins__none">Insights appear once this replay has been read for every player.</p>}
          {insights?.length === 0 && <p className="d-ins__none">Nothing stood out in this match.</p>}
          {insights?.map((i, k) => (
            <button key={i.stat} type="button" className="d-ins__i" onClick={() => onPick(i)}>
              <span className="d-ins__n">{k + 1}</span>
              <span>
                <span className="d-ins__f">{i.fact}</span>
                <span className="d-ins__t">{i.tip}</span>
              </span>
            </button>
          ))}
          {note && <p className="d-ins__note">{note}</p>}
        </div>
        {radar}
      </div>
    </section>
  );
}

function GroupTable({ group, cols, team, flash, bench }: { group: StatGroup; cols: Col[]; team: boolean; flash: string | null; bench: Benchmark | null }) {
  const split = cols.findIndex((c) => c.side === "them");
  const cls = (c: Col, i: number) =>
    [`d-ps__${c.side}`, c.me && "d-ps__me", c.bench && "d-ps__bench", i === split && "d-ps__split"].filter(Boolean).join(" ");
  return (
    <div className="d-ps__card glass" id={`ps-${group.name}`}>
      {bench && group !== KEY_STATS && <GroupRadar group={group} bench={bench} team={team} />}
      <table className="d-ps" aria-labelledby={`ps-cap-${group.name}`}>
        <caption id={`ps-cap-${group.name}`} className="d-k">
          {group.name}
        </caption>
        <thead>
          <tr>
            <th />
            {cols.map((c, i) => (
              <th key={c.id} className={cls(c, i)}>
                {c.name}
                {c.bot && <span className="d-d"> · Bot</span>}
              </th>
            ))}
          </tr>
        </thead>
        <tbody>
          {group.rows
            .filter((r) => team || !r.teamOnly)
            .map((r) => {
              const values = cols.map((c) => statValue(r, c.stats));
              const players = values.filter((_, i) => !cols[i].bench);
              const best = bestValue(players, r.better);
              const max = Math.max(...players.map((v) => v ?? 0), 0);
              const dir = bench ? direction(bench.stats[r.key]) : null;
              return (
                <tr key={r.key} id={`ps-row-${group.name}-${r.key}`} className={flash === r.key && group.name === KEY_STATS.name ? "d-ps__flash" : undefined}>
                  <th scope="row">
                    <Hint gloss={r.gloss}>{r.label}</Hint>
                    {dir && (
                      <span className="d-ps__dir" role="img" aria-label={dir} title={dir}>
                        {dir === "more is better" ? "▲" : "▼"}
                      </span>
                    )}
                  </th>
                  {values.map((v, i) => {
                    const c = cols[i];
                    const total = r.kind === "per5" && !c.bench ? c.stats[r.key] : null;
                    return (
                      <td key={c.id} className={[cls(c, i), v != null && v === best ? "d-ps__best" : ""].join(" ")} title={total != null ? `${Math.round(total)} in the match` : undefined}>
                        {v == null ? "—" : r.format(v)}
                        {!c.bench && r.better !== "none" && v != null && max > 0 && (
                          <span className="d-ps__bar" aria-hidden>
                            <i style={{ width: `${Math.round((v / max) * 100)}%` }} />
                          </span>
                        )}
                      </td>
                    );
                  })}
                </tr>
              );
            })}
        </tbody>
      </table>
    </div>
  );
}
