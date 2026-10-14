import { ChevronDown, ChevronUp, Pencil, Trash2 } from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { modeLabel, timeOfDay } from "@shared/format";
import type { ActivityRow, Neighbour, SessionDetail } from "@shared/generated/types";
import { TERMS } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { act, sentence } from "../act";
import { errorText } from "../api";
import { Confirm } from "../components/Dialog";
import { MenuButton } from "../components/Menu";
import { useLoad } from "../components/useLoad";
import { duration, longDay, percent, plural, span } from "../format";
import type { PageProps } from "../pageProps";
import { CoachReview } from "./CoachReview";
import { EditReviewDialog } from "./EditReviewDialog";
import { MatchPage } from "./MatchPage";
import { SetPackDialog } from "./SetPackDialog";
import { Timeline } from "./SessionTimeline";
import { DEFAULT_TARGET, detailSummary, previousPackId, rateTone } from "./timeline";

const BACKUP_NOTE = "A backup is made first, so you can undo it from Settings → Backups.";

type Open =
  | null
  | { kind: "review" }
  | { kind: "pack"; activity: ActivityRow }
  | { kind: "merge"; with: Neighbour; other: SessionDetail }
  | { kind: "split"; after: ActivityRow; next: ActivityRow }
  | { kind: "delete" }
  | { kind: "delete-activity"; activity: ActivityRow };

interface SessionViewProps extends PageProps {
  id: number;
  /** The session is gone: deleted here, or `vanished` (merged away or deleted elsewhere while
   * shown). The page picks another one and, when it vanished, says so. */
  onGone: (id: number, vanished?: boolean) => void;
  /** Select another session (the survivor of a merge). */
  onSelect: (id: number) => void;
}

/** Loads one session; says so when it no longer exists. */
export function SessionView(props: SessionViewProps) {
  const { api, id, reload, onGone, onChanged } = props;
  const [edits, setEdits] = useState(0);
  const loaded = useLoad(() => api.session(id), `${id}|${reload}|${edits}`);
  useEffect(() => {
    if (loaded.data === null) onGone(id, true);
  }, [loaded.data, onGone, id]);
  // The page shows "This session no longer exists." while it moves to the newest one.
  if (loaded.data === null) return null;
  if (loaded.data === undefined || loaded.data.id !== id) {
    return <p className="d-empty">{loaded.error ?? "Loading…"}</p>;
  }
  const changed = () => {
    setEdits((n) => n + 1);
    onChanged();
  };
  return <SessionBody {...props} detail={loaded.data} changed={changed} />;
}

function activityName(a: ActivityRow): string {
  if (a.kind === "training") return a.pack ? a.pack.name : "Training without a pack";
  if (a.kind === "freeplay") return "Free play";
  const m = a.match_summary;
  return m ? `${modeLabel({ ...m, playlist_id: a.playlist_id })} match` : "Activity";
}

function ReviewStrip({ detail }: { detail: SessionDetail }) {
  const sizes = [...new Set(detail.mmr.map((e) => e.team_size))].sort((a, b) => a - b);
  return (
    <div className="d-review glass">
      {sizes.length === 0 ? (
        <div>
          <div className="d-k">MMR</div>
          <div className="d-mmr__v d-d">—</div>
        </div>
      ) : (
        sizes.map((size) => {
          const rows = detail.mmr.filter((e) => e.team_size === size);
          const row = rows[rows.length - 1];
          return (
            <div key={size}>
              <div className="d-k">{`${size}v${size} MMR`}</div>
              <div className="d-mmr__v">
                {row.mmr}
                {row.change != null && row.change !== 0 && (
                  <span className={row.change > 0 ? "d-g" : "d-r"}>
                    {row.change > 0 ? `+${row.change}` : `−${-row.change}`}
                  </span>
                )}
              </div>
            </div>
          );
        })
      )}
      <div className="d-note">{detail.notes || <span className="d-d">No note</span>}</div>
    </div>
  );
}

function Totals({ detail }: { detail: SessionDetail }) {
  const t = detail.totals;
  const rate = t.attempts > 0 ? t.makes / t.attempts : null;
  const tone = rate == null ? "d-d" : rateTone(rate, t.target_rate ?? DEFAULT_TARGET) === "green" ? "d-g" : "d-a";
  return (
    <div className="d-kpis">
      <div className="d-kpi glass">
        <div className="d-k">
          <Hint gloss={TERMS.kpi_training}>Training</Hint>
        </div>
        <div className="d-kpi__v">
          {t.attempts}
          <small>attempts</small>
        </div>
      </div>
      <div className="d-kpi glass">
        <div className="d-k">
          <Hint gloss={TERMS.kpi_make_rate}>Make rate</Hint>
        </div>
        <div className={`d-kpi__v ${tone}`}>{percent(rate)}</div>
      </div>
      <div className="d-kpi glass">
        <div className="d-k">
          <Hint gloss={TERMS.kpi_ranked}>Ranked</Hint>
        </div>
        <div className="d-kpi__v">
          <span className="d-g">{t.wins}W</span> <span className="d-r">{t.losses}L</span>
        </div>
      </div>
      <div className="d-kpi glass">
        <div className="d-k">
          <Hint gloss={TERMS.kpi_freeplay}>Free play</Hint>
        </div>
        <div className="d-kpi__v">
          {Math.round(t.freeplay_ms / 60_000)}
          <small>min</small>
        </div>
      </div>
    </div>
  );
}

function SessionBox({ s }: { s: SessionDetail }) {
  return (
    <div className="d-box glass">
      <b>{span(s.started_at, s.ended_at)}</b>
      <span className="d-m">{detailSummary(s)}</span>
    </div>
  );
}

type BodyProps = SessionViewProps & { detail: SessionDetail; changed: () => void };

function SessionBody({ api, detail, notify, changed, onSelect, onGone, reload, now }: BodyProps) {
  const [open, setOpen] = useState<Open>(null);
  const [replay, setReplay] = useState<ActivityRow | null>(null);
  const top = useRef<HTMLDivElement>(null);
  const savedScroll = useRef(0);
  const main = () => top.current?.closest(".d-main");
  // The session stays mounted while the match page shows (open groups survive); its scroll is put back.
  useLayoutEffect(() => {
    const el = main();
    if (el) el.scrollTop = replay ? 0 : savedScroll.current;
  }, [replay]);
  function openReplay(activity: ActivityRow) {
    savedScroll.current = main()?.scrollTop ?? 0;
    setReplay(activity);
  }
  const finished = detail.ended_at != null;
  const recording = detail.activities.some((a) => a.ended_at == null);
  const close = () => setOpen(null);
  const length = duration(detail.started_at, detail.ended_at);

  async function openMerge(withSession: Neighbour) {
    const otherId = withSession === "previous" ? detail.previous_id : detail.next_id;
    if (otherId == null) return;
    try {
      const other = await api.session(otherId);
      if (other) setOpen({ kind: "merge", with: withSession, other });
    } catch (err) {
      notify(sentence(errorText(err)), "error");
    }
  }

  async function merge(withSession: Neighbour) {
    try {
      const survivor = await api.mergeSessions(detail.id, withSession);
      notify("Sessions merged", "ok");
      close();
      changed();
      if (survivor !== detail.id) onSelect(survivor);
    } catch (err) {
      notify(sentence(errorText(err)), "error");
    }
  }

  async function split(after: ActivityRow) {
    if (await act(notify, () => api.splitSession(detail.id, after.id), "Session split")) {
      close();
      changed();
    }
  }

  async function deleteSession() {
    if (await act(notify, () => api.deleteSession(detail.id), "Session deleted")) {
      close();
      onGone(detail.id);
    }
  }

  async function deleteActivity(activity: ActivityRow) {
    try {
      const sessionGone = await api.deleteActivity(activity.id);
      notify("Activity deleted", "ok");
      close();
      if (sessionGone) onGone(detail.id);
      else changed();
    } catch (err) {
      notify(sentence(errorText(err)), "error");
    }
  }

  const matches = detail.activities.filter((a) => a.match_summary != null).length;
  const mergeOrder = (other: SessionDetail) =>
    detail.started_at <= other.started_at ? [detail, other] : [other, detail];
  return (
    <>
      {replay && <MatchPage api={api} activity={replay} onBack={() => setReplay(null)} />}
      <div ref={top} hidden={replay != null}>
        <div className="d-hd">
          <div>
            <h1>{longDay(detail.started_at)}</h1>
            <div className="d-meta">
              {span(detail.started_at, detail.ended_at)}
              {length && ` · ${length}`}
            </div>
          </div>
          <div className="d-btns">
            <button type="button" className="d-btn" disabled={recording} onClick={() => setOpen({ kind: "review" })}>
              <Pencil className="d-icon" aria-hidden />
              Edit review
            </button>
            <MenuButton
              label="Session actions"
              items={[
                {
                  label: "Merge with previous session",
                  icon: <ChevronUp className="d-icon" aria-hidden />,
                  disabled: !finished || detail.previous_id == null,
                  onSelect: () => void openMerge("previous"),
                },
                {
                  label: "Merge with next session",
                  icon: <ChevronDown className="d-icon" aria-hidden />,
                  disabled: !finished || detail.next_id == null,
                  onSelect: () => void openMerge("next"),
                },
                {
                  label: "Delete session…",
                  icon: <Trash2 className="d-icon" aria-hidden />,
                  danger: true,
                  separatorBefore: true,
                  disabled: !finished,
                  onSelect: () => setOpen({ kind: "delete" }),
                },
              ]}
            />
          </div>
        </div>
        <ReviewStrip detail={detail} />
        <Totals detail={detail} />
        <CoachReview api={api} sessionId={detail.id} reload={reload} />
        <Timeline
          detail={detail}
          now={now}
          onSetPack={(activity) => setOpen({ kind: "pack", activity })}
          onSplit={(after, next) => setOpen({ kind: "split", after, next })}
          onDelete={(activity) => setOpen({ kind: "delete-activity", activity })}
          onOpenReplay={openReplay}
        />
      </div>
      {open?.kind === "review" && (
        <EditReviewDialog api={api} detail={detail} notify={notify} onDone={changed} onClose={close} />
      )}
      {open?.kind === "pack" && (
        <SetPackDialog
          api={api}
          activity={open.activity}
          suggestedPackId={previousPackId(detail.activities, detail.activities.indexOf(open.activity))}
          notify={notify}
          onDone={changed}
          onClose={close}
        />
      )}
      {open?.kind === "merge" && (
        <Confirm
          title="Merge these two sessions?"
          lead={(() => {
            const [first, second] = mergeOrder(open.other);
            return `They become one session from ${timeOfDay(first.started_at)} to ${timeOfDay(second.ended_at ?? second.started_at)}.`;
          })()}
          body={
            <>
              <div className="d-cmp">
                <SessionBox s={mergeOrder(open.other)[0]} />
                <span className="d-m" aria-hidden>
                  +
                </span>
                <SessionBox s={mergeOrder(open.other)[1]} />
              </div>
              <p>Both notes are kept, one after the other. {BACKUP_NOTE}</p>
            </>
          }
          confirm="Merge"
          onConfirm={() => merge(open.with)}
          onClose={close}
        />
      )}
      {open?.kind === "split" && (
        <Confirm
          title="Split this session here?"
          lead={`${span(detail.started_at, open.after.ended_at)} and ${span(open.next.started_at, detail.ended_at)}`}
          body={<p>Everything after the gap becomes a new session, with the MMR you entered after it. {BACKUP_NOTE}</p>}
          confirm="Split"
          onConfirm={() => split(open.after)}
          onClose={close}
        />
      )}
      {open?.kind === "delete" && (
        <Confirm
          title="Delete this session?"
          lead={`${longDay(detail.started_at)}, ${span(detail.started_at, detail.ended_at)}`}
          body={
            <p>
              Its {plural(detail.activities.length, "activity", "activities")}, {plural(detail.totals.attempts, "attempt")},{" "}
              {plural(matches, "match", "matches")} and the MMR you entered are removed. {BACKUP_NOTE}
            </p>
          }
          confirm="Delete"
          danger
          onConfirm={deleteSession}
          onClose={close}
        />
      )}
      {open?.kind === "delete-activity" && (
        <Confirm
          title="Delete this activity?"
          lead={`${activityName(open.activity)} · ${span(open.activity.started_at, open.activity.ended_at)}`}
          body={
            <p>
              It is removed with everything recorded in it; if it is the session's last activity, the session goes too.{" "}
              {BACKUP_NOTE}
            </p>
          }
          confirm="Delete"
          danger
          onConfirm={() => deleteActivity(open.activity)}
          onClose={close}
        />
      )}
    </>
  );
}
