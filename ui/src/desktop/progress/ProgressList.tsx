import { longDuration } from "@shared/format";
import type { PackLane, ProgressList as ListData, ProgressRange } from "@shared/generated/types";
import { useState, type ReactNode } from "react";
import { Sparkline } from "../charts/Sparkline";
import { percent, plural } from "../format";
import { rateTone } from "../sessions/timeline";
import { changeTone, MODE_COLORS, modeName, signed, winPct } from "./format";
import type { Selection } from "./ProgressPage";
import { RangeChips } from "./RangeChips";
import { Select } from "../components/Select";

type PackSort = "played" | "change" | "name";

const PACK_SORTS: { value: PackSort; label: string }[] = [
  { value: "played", label: "Played in range" },
  { value: "change", label: "Biggest change" },
  { value: "name", label: "Name" },
];

export function sortPacks(packs: PackLane[], sort: PackSort): PackLane[] {
  const list = [...packs];
  if (sort === "name") return list.sort((a, b) => a.pack.name.localeCompare(b.pack.name));
  if (sort === "change") return list.sort((a, b) => Math.abs(b.change ?? 0) - Math.abs(a.change ?? 0));
  return list; // the read sends them by attempts
}

interface ProgressListProps {
  data: ListData | undefined;
  range: ProgressRange;
  onRange: (r: ProgressRange) => void;
  selection: Selection;
  onSelect: (s: Selection) => void;
}

export function ProgressList({ data, range, onRange, selection, onSelect }: ProgressListProps) {
  const [sort, setSort] = useState<PackSort>("played");
  const on = (s: Selection) =>
    s.kind === selection.kind &&
    (s.kind !== "mode" || (selection.kind === "mode" && s.teamSize === selection.teamSize)) &&
    (s.kind !== "pack" || (selection.kind === "pack" && s.id === selection.id));
  const lane = (s: Selection, children: ReactNode) => (
    <button type="button" className={`d-lane p-lane${on(s) ? " d-lane--on" : ""}`} aria-current={on(s) ? "true" : undefined} onClick={() => onSelect(s)}>
      {children}
    </button>
  );
  return (
    <nav className="d-list" aria-label="Progress">
      <div className="p-head">
        <h2>Progress</h2>
        <RangeChips range={range} onRange={onRange} />
      </div>
      {data == null ? (
        <p className="d-empty">Loading…</p>
      ) : (
        <>
          {lane(
            { kind: "overview" },
            <span className="p-lane__main">
              <span className="d-lane__t">Overview</span>
              <span className="d-lane__sub">
                {plural(data.sessions, "session")} · {longDuration(data.played_ms)}
              </span>
            </span>,
          )}
          {data.modes.length > 0 && <div className="p-grp">Ranked</div>}
          {data.modes.map((m) =>
            lane(
              { kind: "mode", teamSize: m.team_size },
              <>
                <span className="p-lane__main">
                  <span className="d-lane__t">{modeName(m.team_size)}</span>
                  <span className="d-lane__sub">
                    {plural(m.matches, "match", "matches")} · {winPct(m.wins, m.losses) ?? "—"}% won
                  </span>
                </span>
                <Sparkline values={m.mmr_spark} color={MODE_COLORS[m.team_size]} />
                <span className="p-lane__r">
                  <b>{m.mmr_latest ?? "—"}</b>
                  <span className={changeTone(m.mmr_change)}>{signed(m.mmr_change)}</span>
                </span>
              </>,
            ),
          )}
          {data.packs.length > 0 && (
            <div className="p-grp">
              Packs
              <Select label="Sort packs" variant="inline" value={sort} options={PACK_SORTS} onChange={setSort} />
            </div>
          )}
          {sortPacks(data.packs, sort).map((p) => {
            const tone = rateTone(p.rate, p.pack.target_rate) === "green" ? "d-g" : "d-a";
            return lane(
              { kind: "pack", id: p.pack.id },
              <>
                <span className="p-lane__main">
                  <span className="d-lane__t">{p.pack.name}</span>
                  <span className="d-lane__sub">
                    {plural(p.sessions, "session")} · {p.attempts} attempts
                  </span>
                </span>
                <Sparkline values={p.spark} color={tone === "d-g" ? "#6fe3a9" : "#f2b46b"} />
                <span className="p-lane__r">
                  <b className={tone}>{percent(p.rate)}</b>
                  <span className={changeTone(p.change)}>{p.change == null ? "—" : signed(p.change * 100)}</span>
                </span>
              </>,
            );
          })}
        </>
      )}
    </nav>
  );
}
