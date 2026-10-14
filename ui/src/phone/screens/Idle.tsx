import { Glass, Kv, Pill } from "@shared/components/ui";
import { dayLabel, longDuration } from "@shared/format";
import { Header, sessionPill } from "../Header";
import type { ScreenProps } from "../screenProps";
import { SessionStats } from "./SessionStats";

export function Idle({ snap, conn, now }: ScreenProps) {
  // While a session is open (menus, other modes) it is "this session"; else the last one.
  const current = snap.live.current_session;
  const s = current ?? snap.live.last_session;
  const inOtherMode = snap.live.block?.kind === "other";
  const left = inOtherMode ? (
    <Pill live>In another mode</Pill>
  ) : snap.game_running ? (
    <Pill live>Waiting for a game</Pill>
  ) : (
    <Pill off>Rocket League closed</Pill>
  );
  const mmr = (size: number) => snap.live.latest_mmr.find((m) => m.team_size === size)?.mmr ?? "—";
  return (
    <div className="screen">
      <Header conn={conn} left={left} right={current ? sessionPill(snap, now) : <Pill>Paired ✓</Pill>} />
      {s ? (
        <>
          <h1 className="title">{current ? "This session" : "Last session"}</h1>
          <p className="subtitle">
            {dayLabel(s.started_at, now)} · {longDuration((current ? now : s.ended_at) - s.started_at)}
          </p>
          <SessionStats s={s} />
          <Glass className="kv-card">
            <Kv label="MMR 1v1" value={mmr(1)} />
            <Kv label="MMR 2v2" value={mmr(2)} />
            <Kv label="MMR 3v3" value={mmr(3)} />
          </Glass>
          {s.notes && <Glass className="kv-card note-card">“{s.notes}”</Glass>}
        </>
      ) : (
        <>
          <h1 className="title">No sessions yet</h1>
          <p className="pairing__text">Start Rocket League and play.</p>
        </>
      )}
    </div>
  );
}
