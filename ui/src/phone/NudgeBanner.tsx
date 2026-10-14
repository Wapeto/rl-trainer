import { X } from "lucide-react";
import { useEffect, useState } from "react";
import type { Nudge } from "@shared/generated/types";
import { vibrate } from "@shared/vibrate";

const BUZZED_KEY = "rl-nudge-buzzed";

interface KeyStore {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

function safeStorage(): KeyStore | null {
  try {
    return window.sessionStorage;
  } catch {
    return null;
  }
}

export function nudgeText(n: Nudge): {
  kind: string;
  line: string;
  advice: string;
} {
  if (n.kind === "rate_drop") {
    const pct = (r: number) => Math.round(r * 100);
    return {
      kind: "Rate dropping",
      line: `Last 20: ${pct(n.last20_rate)}% · your best today: ${pct(n.best20_rate)}%`,
      advice: "Take a 2-min break or switch pack.",
    };
  }
  return {
    kind: "Miss streak",
    line: `${n.count} misses in a row`,
    advice: "Reset your head: slow the first touch, or switch pack.",
  };
}

interface NudgeBannerProps {
  nudge: Nudge | null | undefined;
  buzz?: (ms: number) => void;
  storage?: KeyStore | null;
}

/** The coach's nudge over the top of the Training screen (spec §3.3, mockup A). */
export function NudgeBanner({ nudge, buzz = vibrate, storage = safeStorage() }: NudgeBannerProps) {
  const [dismissed, setDismissed] = useState<number | null>(null);
  const id = nudge?.fired_at_attempt ?? null;
  useEffect(() => {
    if (id == null) return;
    if (storage?.getItem(BUZZED_KEY) === String(id)) return;
    storage?.setItem(BUZZED_KEY, String(id));
    buzz(200);
  }, [id, buzz, storage]);
  if (!nudge || dismissed === id) return null;
  const t = nudgeText(nudge);
  return (
    <div className="nudge" role="status">
      <span className="nudge__k">{t.kind}</span>
      <b className="nudge__t">{t.line}</b>
      <span className="nudge__s">{t.advice}</span>
      <button type="button" className="nudge__x" aria-label="Dismiss" onClick={() => setDismissed(id)}>
        <X aria-hidden size={16} />
      </button>
    </div>
  );
}
