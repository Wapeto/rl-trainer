import { Search } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import type { Notify } from "@shared/components/Toast";
import type { ActivityRow, PackStats } from "@shared/generated/types";
import { act, sentence } from "../act";
import { errorText, type DesktopApi } from "../api";
import { Dialog } from "../components/Dialog";
import { useDebounced } from "../components/useLoad";
import { span } from "../format";

interface SetPackDialogProps {
  api: DesktopApi;
  activity: ActivityRow;
  /** The pack of the previous training activity in the session, listed first. */
  suggestedPackId: number | null;
  notify: Notify;
  onDone: () => void;
  onClose: () => void;
}

export function SetPackDialog({ api, activity, suggestedPackId, notify, onDone, onClose }: SetPackDialogProps) {
  const [packs, setPacks] = useState<PackStats[] | null>(null);
  const [query, setQuery] = useState("");
  const [choice, setChoice] = useState<number | null>(activity.pack?.id ?? suggestedPackId);
  const [busy, setBusy] = useState(false);

  const search = useDebounced(query, 200);
  const currentId = activity.pack?.id ?? null;

  useEffect(() => {
    let current = true;
    api.packs("name", search).then(
      (list) => current && setPacks(list.filter((p) => !p.pack.hidden || p.pack.id === currentId)),
      (err: unknown) => current && notify(sentence(errorText(err)), "error"),
    );
    return () => {
      current = false;
    };
  }, [api, notify, search, currentId]);

  const shown = useMemo(() => {
    const list = packs ?? [];
    const i = list.findIndex((p) => p.pack.id === suggestedPackId);
    return i > 0 ? [list[i], ...list.slice(0, i), ...list.slice(i + 1)] : list;
  }, [packs, suggestedPackId]);

  async function save(packId: number | null) {
    setBusy(true);
    const ok = await act(notify, () => api.setBlockPack(activity.id, packId), packId == null ? "Pack cleared" : "Pack set");
    setBusy(false);
    if (ok) {
      onDone();
      onClose();
    }
  }

  return (
    <Dialog
      title="Which pack was this?"
      lead={`${activity.attempts} attempts · ${span(activity.started_at, activity.ended_at)}`}
      onClose={onClose}
    >
      <label className="d-search glass">
        <Search className="d-icon" aria-hidden />
        <input
          type="search"
          aria-label="Search packs"
          placeholder="Search name or code"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      <div className="d-picklist" role="listbox" aria-label="Packs">
        {packs === null && <p className="d-empty">Loading…</p>}
        {packs !== null && shown.length === 0 && <p className="d-empty">No pack matches.</p>}
        {shown.map((p) => (
          <button
            key={p.pack.id}
            type="button"
            role="option"
            aria-selected={p.pack.id === choice}
            className={`d-pick${p.pack.id === choice ? " d-pick--on" : ""}`}
            onClick={() => setChoice(p.pack.id)}
          >
            <span className="d-pick__main">
              <b>{p.pack.name}</b>
              <span className="d-code">{p.pack.code}</span>
              {p.pack.hidden && <span className="d-tag">Hidden</span>}
            </span>
            {p.pack.id === suggestedPackId && <span className="d-g d-s">Played before this</span>}
          </button>
        ))}
      </div>
      <div className="d-actions">
        <button type="button" className="d-btn" disabled={busy} onClick={() => save(null)}>
          No pack
        </button>
        <button type="button" className="d-btn" onClick={onClose}>
          Cancel
        </button>
        <button type="button" className="d-btn d-btn--primary" disabled={busy || choice == null} onClick={() => save(choice)}>
          Set
        </button>
      </div>
    </Dialog>
  );
}
