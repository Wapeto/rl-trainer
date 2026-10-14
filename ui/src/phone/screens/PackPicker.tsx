import { Search } from "lucide-react";
import { useEffect, useState } from "react";
import type { Api } from "@shared/api/client";
import { Sheet } from "@shared/components/Sheet";
import type { Notify } from "@shared/components/Toast";
import type { PackRow, PackStats } from "@shared/generated/types";
import { act } from "../act";

interface PackPickerProps {
  open: boolean;
  /** The block shown now; read at tap time so a split while the sheet is open is respected. */
  blockId: number;
  currentPackId: number | null;
  suggestion: PackRow | null;
  api: Api | null;
  notify: Notify;
  onClose: () => void;
}

export function PackPicker({ open, blockId, currentPackId, suggestion, api, notify, onClose }: PackPickerProps) {
  const [packs, setPacks] = useState<PackStats[] | null>(null);
  const [query, setQuery] = useState("");

  const [search, setSearch] = useState("");
  useEffect(() => {
    const id = setTimeout(() => setSearch(query), 200);
    return () => clearTimeout(id);
  }, [query]);

  useEffect(() => {
    if (!open || !api) return;
    let current = true;
    api
      .packs(search)
      .then((list) => current && setPacks(list))
      .catch(() => notify("Couldn't load the packs from the PC.", "error"));
    return () => {
      current = false;
    };
  }, [open, api, notify, search]);

  async function choose(packId: number | null) {
    if (!api) return;
    const ok = await act(notify, () => api.setPack(blockId, packId), "Pack set");
    // On failure the sheet stays open so the player can try again.
    if (!ok) return;
    setQuery("");
    onClose();
  }

  const shown = packs ?? [];
  return (
    <Sheet open={open} title="Choose pack" onClose={onClose}>
      <label className="search glass">
        <Search size={18} aria-hidden />
        <input
          type="search"
          placeholder="Search name, code or tag"
          aria-label="Search packs"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
      </label>
      {suggestion && (
        <button type="button" className="suggest" onClick={() => choose(suggestion.id)}>
          <span className="suggest__k">Still on</span>
          <span className="suggest__t">{suggestion.name}</span>
        </button>
      )}
      <ul className="plist glass">
        {packs === null && <li className="pitem pitem--muted">Loading…</li>}
        {shown.map((p) => (
          <li key={p.pack.id}>
            <button
              type="button"
              className={`pitem${p.pack.id === currentPackId ? " pitem--current" : ""}`}
              onClick={() => choose(p.pack.id)}
            >
              <span className="pitem__main">
                <span className="pitem__name">{p.pack.name}</span>
                <span className="pitem__meta">{[p.pack.code, ...p.pack.tags].join(" · ")}</span>
              </span>
              <span className="pitem__rate">{p.make_rate == null ? "—" : `${Math.round(p.make_rate * 100)}%`}</span>
            </button>
          </li>
        ))}
        <li>
          <button type="button" className="pitem pitem--unassigned" onClick={() => choose(null)}>
            <span className="pitem__name">Unassigned</span>
          </button>
        </li>
      </ul>
    </Sheet>
  );
}
