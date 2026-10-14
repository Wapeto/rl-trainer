import type { ReactNode } from "react";
import type { ConnState } from "@shared/api/liveStore";
import { Pill } from "@shared/components/ui";
import type { AppSnapshot } from "@shared/generated/types";
import { hoursMinutes } from "@shared/format";

export function Header({ conn, left, right }: { conn: ConnState; left: ReactNode; right: ReactNode }) {
  return (
    <header className="header">
      {left}
      {conn === "reconnecting" ? <Pill>Reconnecting…</Pill> : right}
    </header>
  );
}

export function sessionPill(snap: AppSnapshot, now: number): ReactNode {
  const session = snap.live.session;
  return session ? <Pill>Session {hoursMinutes(now - session.started_at)}</Pill> : null;
}
