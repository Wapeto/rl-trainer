import { Copy, Eye, EyeOff, Pencil } from "lucide-react";
import type { ReactNode } from "react";
import { dayLabel, timeOfDay } from "@shared/format";
import type { PackRow } from "@shared/generated/types";
import { type Gloss, PROGRESS } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { act } from "../act";
import { useLoad } from "../components/useLoad";
import { percent, plural } from "../format";
import type { PageProps } from "../pageProps";
import { rateTone } from "../sessions/timeline";

const toneClass = (rate: number | null | undefined, target: number) =>
  rate == null ? "d-d" : rateTone(rate, target) === "green" ? "d-g" : "d-a";

interface KpiProps {
  label: string;
  gloss: Gloss;
  value: ReactNode;
  className?: string;
  unit?: string;
}

function Kpi({ label, gloss, value, className = "", unit }: KpiProps) {
  return (
    <div className="d-kpi glass">
      <div className="d-k">
        <Hint gloss={gloss}>{label}</Hint>
      </div>
      <div className={`d-kpi__v ${className}`.trim()}>
        {value}
        {unit && <small>{unit}</small>}
      </div>
    </div>
  );
}

interface PackViewProps extends PageProps {
  id: number;
  /** Bumped by the page after an edit, so the pack reloads. */
  edits: number;
  onEdit: (pack: PackRow) => void;
  changed: () => void;
}

export function PackView({ api, id, reload, edits, now, notify, onEdit, changed }: PackViewProps) {
  const loaded = useLoad(() => api.pack(id), `${id}|${reload}|${edits}`);
  const d = loaded.data;
  if (d === null) return <p className="d-empty">This pack no longer exists.</p>;
  if (d === undefined || d.stats.pack.id !== id) return <p className="d-empty">{loaded.error ?? "Loading…"}</p>;
  const p = d.stats.pack;

  async function copy() {
    try {
      await navigator.clipboard.writeText(p.code);
      notify("Code copied", "ok");
    } catch {
      notify("Couldn't copy the code", "error");
    }
  }

  async function toggleHidden() {
    const ok = await act(notify, () => api.setPackHidden(p.id, !p.hidden), p.hidden ? "Pack shown again" : "Pack hidden");
    if (ok) changed();
  }

  return (
    <>
      <div className="d-hd">
        <div>
          <h1>{p.name}</h1>
          <div className="d-meta">
            <span className="d-code">{p.code}</span>
            <button type="button" className="d-btn d-btn--icon" aria-label="Copy the code" onClick={copy}>
              <Copy className="d-icon" aria-hidden />
            </button>
            {p.shot_count != null && <span>· {plural(p.shot_count, "shot")}</span>}
            {p.hidden && <span className="d-tag">Hidden</span>}
          </div>
          {p.tags.length > 0 && (
            <div className="d-chips">
              {p.tags.map((t) => (
                <span key={t} className="d-chip">
                  {t}
                </span>
              ))}
            </div>
          )}
        </div>
        <div className="d-btns">
          <button type="button" className="d-btn" onClick={() => onEdit(p)}>
            <Pencil className="d-icon" aria-hidden />
            Edit
          </button>
          <button type="button" className="d-btn" onClick={toggleHidden}>
            {p.hidden ? <Eye className="d-icon" aria-hidden /> : <EyeOff className="d-icon" aria-hidden />}
            {p.hidden ? "Unhide" : "Hide"}
          </button>
        </div>
      </div>
      <div className="d-kpis d-kpis--top">
        <Kpi
          label="Overall"
          gloss={PROGRESS.pack_overall}
          value={percent(d.stats.make_rate)}
          className={toneClass(d.stats.make_rate, p.target_rate)}
          unit={`target ${percent(p.target_rate)}`}
        />
        <Kpi label="Last session" gloss={PROGRESS.pack_last_session} value={percent(d.last_session_rate)} className={toneClass(d.last_session_rate, p.target_rate)} />
        <Kpi label="Best session" gloss={PROGRESS.best_session} value={percent(d.best_session_rate)} className={toneClass(d.best_session_rate, p.target_rate)} />
        <Kpi label="Played" gloss={PROGRESS.pack_played} value={d.stats.attempts} unit="attempts" />
      </div>
      <div className="d-panel glass">
        <div className="d-panel__h">
          <Hint gloss={PROGRESS.session_by_session}>
            <span className="d-k">Session by session</span>
          </Hint>
          <span className="d-m d-s">White tick = target</span>
        </div>
        {d.history.length === 0 && <p className="d-empty">Not played yet.</p>}
        {d.history.map((h) => {
          const green = rateTone(h.make_rate, p.target_rate) === "green";
          return (
            <div key={h.session_id} className="d-hr">
              <span>
                {dayLabel(h.started_at, now)} {timeOfDay(h.started_at)}
              </span>
              <span className="d-m">{plural(h.attempts, "attempt")}</span>
              <div className="d-bar" role="img" aria-label={`${percent(h.make_rate)} against a ${percent(p.target_rate)} target`}>
                <i style={{ width: percent(h.make_rate), background: green ? "var(--green)" : "var(--amber)" }} />
                <b style={{ left: percent(p.target_rate) }} />
              </div>
              <span className={green ? "d-g" : "d-a"}>{percent(h.make_rate)}</span>
            </div>
          );
        })}
      </div>
      {p.notes && (
        <div className="d-panel glass">
          <div className="d-panel__h">
            <span className="d-k">Note</span>
          </div>
          <p className="d-panel__body">{p.notes}</p>
        </div>
      )}
    </>
  );
}
