import { Car, ChevronDown, ChevronRight, CircleDashed, Scissors, Trash2, Trophy } from "lucide-react";
import { Fragment, useState } from "react";
import { Ring } from "@shared/components/Ring";
import { arenaName, kmh, modeLabel, timeOfDay } from "@shared/format";
import type { ActivityRow, SessionDetail } from "@shared/generated/types";
import { MenuButton } from "../components/Menu";
import { duration, percent, plural } from "../format";
import { ReplayCell } from "./ReplayCell";
import { DEFAULT_TARGET, canSplit, itemFirst, itemLast, rateTone, timelineItems, type TimelineItem } from "./timeline";

export interface TimelineActions {
  onSetPack: (activity: ActivityRow) => void;
  onSplit: (after: ActivityRow, next: ActivityRow) => void;
  onDelete: (activity: ActivityRow) => void;
  onOpenReplay: (activity: ActivityRow) => void;
}

interface TimelineProps extends TimelineActions {
  detail: SessionDetail;
  now: number;
}

/** The session's activities on a line; a finished session offers a split between cards. */
export function Timeline({ detail, now, onSetPack, onSplit, onDelete, onOpenReplay }: TimelineProps) {
  const finished = detail.ended_at != null;
  const items = timelineItems(detail.activities);
  if (items.length === 0) return <p className="d-empty">Nothing was recorded in this session.</p>;
  return (
    <div className="d-tl">
      {items.map((item, i) => (
        <Fragment key={itemFirst(item).id}>
          {i > 0 && finished && canSplit(itemLast(items[i - 1]), itemFirst(item)) && (
            <Gap onSplit={() => onSplit(itemLast(items[i - 1]), itemFirst(item))} />
          )}
          {item.kind === "single" ? (
            <ActivityCard activity={item.activity} finished={finished} onSetPack={onSetPack} onDelete={onDelete} />
          ) : (
            <MatchGroup item={item} finished={finished} now={now} onDelete={onDelete} onOpenReplay={onOpenReplay} />
          )}
        </Fragment>
      ))}
    </div>
  );
}

function Gap({ onSplit }: { onSplit: () => void }) {
  return (
    <div className="d-gap">
      <button type="button" className="d-gap__btn" onClick={onSplit}>
        <Scissors className="d-icon" aria-hidden />
        Split here
      </button>
    </div>
  );
}

interface CardProps {
  activity: ActivityRow;
  finished: boolean;
  onSetPack: (activity: ActivityRow) => void;
  onDelete: (activity: ActivityRow) => void;
}

function ActivityCard({ activity: a, finished, onSetPack, onDelete }: CardProps) {
  const ended = a.ended_at != null;
  const length = duration(a.started_at, a.ended_at);
  const menu = finished && (
    <MenuButton
      label="Activity actions"
      items={[{ label: "Delete activity…", icon: <Trash2 className="d-icon" aria-hidden />, danger: true, onSelect: () => onDelete(a) }]}
    />
  );
  if (a.kind === "training") {
    const rate = a.attempts > 0 ? a.makes / a.attempts : null;
    const tone = rateTone(rate, a.pack?.target_rate ?? DEFAULT_TARGET);
    return (
      <div className={`d-ev glass ${a.pack ? "d-ev--training" : "d-ev--unassigned"}`}>
        <span className="d-ev__time">{timeOfDay(a.started_at)}</span>
        <div className="d-ev__row">
          <span className="d-kind">
            <Ring value={rate ?? 0} tone={tone} size="mini" label={`Make rate ${percent(rate)}`} />
          </span>
          <div className="d-ev__body">
            <div className={`d-ev__t${a.pack ? "" : " d-a"}`}>{a.pack ? a.pack.name : "No pack set"}</div>
            <div className="d-ev__sub">
              <span className={tone === "green" ? "d-g" : "d-a"}>{percent(rate)}</span>
              {` · ${a.makes} of ${a.attempts}`}
              {length && ` · ${length}`}
            </div>
          </div>
          {ended ? (
            <button type="button" className={`d-btn${a.pack ? "" : " d-btn--warn"}`} onClick={() => onSetPack(a)}>
              {a.pack ? "Change" : "Set pack"}
            </button>
          ) : (
            <span className="d-now">NOW</span>
          )}
          {menu}
        </div>
      </div>
    );
  }
  const free = a.kind === "freeplay";
  const Icon = free ? Car : CircleDashed;
  const title = free ? "Free play" : a.kind === "match" ? "Match" : "Other";
  const sub = [length, free && a.freeplay ? `hardest hit ${kmh(a.freeplay.max_hit_speed)} km/h` : null]
    .filter(Boolean)
    .join(" · ");
  return (
    <div className="d-ev glass">
      <span className="d-ev__time">{timeOfDay(a.started_at)}</span>
      <div className="d-ev__row">
        <span className="d-kind">
          <Icon className="d-icon" aria-hidden />
        </span>
        <div className="d-ev__body">
          <div className="d-ev__t">{title}</div>
          {sub && <div className="d-ev__sub">{sub}</div>}
        </div>
        {!ended && <span className="d-now">NOW</span>}
        {menu}
      </div>
    </div>
  );
}

export function resultText(result: string, us: number, them: number): string {
  const score = `${us}–${them}`;
  if (result === "win") return `W ${score}`;
  if (result === "loss") return `L ${score}`;
  return score;
}

interface MatchGroupProps {
  item: Extract<TimelineItem, { kind: "matches" }>;
  finished: boolean;
  now: number;
  onDelete: (activity: ActivityRow) => void;
  onOpenReplay: (activity: ActivityRow) => void;
}

function MatchGroup({ item, finished, now, onDelete, onOpenReplay }: MatchGroupProps) {
  const [open, setOpen] = useState(false);
  const first = item.activities[0];
  const last = item.activities[item.activities.length - 1];
  const results = item.activities.flatMap((a) => (a.match_summary ? [a.match_summary] : []));
  const wins = results.filter((m) => m.result === "win").length;
  const losses = results.filter((m) => m.result === "loss").length;
  const length = duration(first.started_at, last.ended_at);
  const live = item.activities.some((a) => a.ended_at == null);
  return (
    <div className="d-ev glass d-ev--match">
      <span className="d-ev__time">{timeOfDay(first.started_at)}</span>
      <div className="d-ev__row">
        <span className="d-kind d-kind--match">
          <Trophy className="d-icon" aria-hidden />
        </span>
        <div className="d-ev__body">
          <div className="d-ev__t">
            {modeLabel({ ranked: item.ranked, team_size: item.teamSize, playlist_id: item.playlistId })} · <span className="d-g">{wins}W</span>{" "}
            <span className="d-r">{losses}L</span>
          </div>
          <div className="d-ev__sub">
            {plural(item.activities.length, "match", "matches")}
            {length && ` · ${length}`}
          </div>
        </div>
        {live && <span className="d-now">NOW</span>}
        <button
          type="button"
          className="d-btn d-btn--icon"
          aria-expanded={open}
          aria-label={open ? "Hide matches" : "Show matches"}
          onClick={() => setOpen(!open)}
        >
          {open ? <ChevronDown className="d-icon" aria-hidden /> : <ChevronRight className="d-icon" aria-hidden />}
        </button>
      </div>
      {open && (
        <table className="d-mt">
          <colgroup>
            <col style={{ width: 80 }} />
            <col />
            <col style={{ width: 66 }} />
            <col style={{ width: 66 }} />
            <col style={{ width: 70 }} />
            <col style={{ width: 62 }} />
            <col style={{ width: 132 }} />
            {finished && <col style={{ width: 44 }} />}
          </colgroup>
          <thead>
            <tr>
              <th>Result</th>
              <th>Arena</th>
              <th className="n">Score</th>
              <th className="n">Goals</th>
              <th className="n">Assists</th>
              <th className="n">Saves</th>
              <th>Replay</th>
              {finished && <th aria-label="Delete" />}
            </tr>
          </thead>
          <tbody>
            {item.activities.map((a) => {
              const m = a.match_summary;
              if (!m) return null;
              const arena = arenaName(m.arena);
              const tone = m.result === "win" ? "d-g" : m.result === "loss" ? "d-r" : "d-m";
              return (
                <Fragment key={a.id}>
                  <tr>
                    <td className={`d-res ${tone}`}>{resultText(m.result, m.team_score, m.opponent_score)}</td>
                    <td title={arena}>
                      {arena}
                      {m.overtime && <span className="d-d"> · OT</span>}
                    </td>
                    <td className="n">{m.score}</td>
                    <td className="n">{m.goals}</td>
                    <td className="n">{m.assists}</td>
                    <td className="n">{m.saves}</td>
                    <td>
                      <ReplayCell match={m} startedAt={a.started_at} now={now} onOpen={() => onOpenReplay(a)} />
                    </td>
                    {finished && (
                      <td className="n">
                        <button
                          type="button"
                          className="d-btn d-btn--icon"
                          aria-label={`Delete the match on ${arena}`}
                          onClick={() => onDelete(a)}
                        >
                          <Trash2 className="d-icon" aria-hidden />
                        </button>
                      </td>
                    )}
                  </tr>
                </Fragment>
              );
            })}
          </tbody>
        </table>
      )}
    </div>
  );
}
