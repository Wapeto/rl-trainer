import { Plus, Search } from "lucide-react";
import { useCallback, useState } from "react";
import { Ring } from "@shared/components/Ring";
import { dayLabel } from "@shared/format";
import type { PackRow, PackSort, PackStats } from "@shared/generated/types";
import { useDebounced, useLoad } from "../components/useLoad";
import { percent } from "../format";
import type { PageProps } from "../pageProps";
import { rateTone } from "../sessions/timeline";
import { PackDialog } from "./PackDialog";
import { PackView } from "./PackView";
import { Select } from "../components/Select";

const SORTS: { value: PackSort; label: string }[] = [
  { value: "recent", label: "Recently played" },
  { value: "name", label: "Name" },
  { value: "make_rate", label: "Make rate" },
  { value: "most_played", label: "Most played" },
];

function PackLane({ stats, on, now, onSelect }: { stats: PackStats; on: boolean; now: number; onSelect: (id: number) => void }) {
  const p = stats.pack;
  const tone = rateTone(stats.make_rate, p.target_rate);
  return (
    <button
      type="button"
      className={`d-lane d-lane--pack${on ? " d-lane--on" : ""}`}
      aria-current={on ? "true" : undefined}
      onClick={() => onSelect(p.id)}
    >
      <span className="d-kind d-kind--sm">
        <Ring value={stats.make_rate ?? 0} tone={tone} size="mini" label={`Make rate ${percent(stats.make_rate)}`} />
      </span>
      <span>
        <span className="d-lane__t">{p.name}</span>
        <span className="d-lane__sub">
          {stats.last_trained_at != null ? `Last played ${dayLabel(stats.last_trained_at, now)}` : "Never played"}
        </span>
      </span>
      <span className={`d-lane__r ${stats.make_rate == null ? "" : tone === "green" ? "d-g" : "d-a"}`}>
        {percent(stats.make_rate)}
      </span>
    </button>
  );
}

export function PacksPage(props: PageProps) {
  const { api, reload, now, notify, onChanged } = props;
  const [query, setQuery] = useState("");
  const search = useDebounced(query, 200);
  const [sort, setSort] = useState<PackSort>("recent");
  const [edits, setEdits] = useState(0);
  const list = useLoad(() => api.packs(sort, search), `${sort}|${search}|${reload}|${edits}`);
  const [selected, setSelected] = useState<number | null>(() => props.focusId ?? null);
  const [showHidden, setShowHidden] = useState(false);
  const [dialog, setDialog] = useState<{ pack: PackRow | null } | null>(null);
  const changed = useCallback(() => {
    setEdits((n) => n + 1);
    onChanged();
  }, [onChanged]);

  const packs = list.data ?? [];
  const visible = packs.filter((p) => !p.pack.hidden);
  const hidden = packs.filter((p) => p.pack.hidden);
  const selectedId = selected ?? visible[0]?.pack.id ?? null;
  const lane = (p: PackStats) => (
    <PackLane key={p.pack.id} stats={p} on={p.pack.id === selectedId} now={now} onSelect={setSelected} />
  );

  return (
    <>
      <section className="d-list" aria-label="Packs">
        <div className="d-list__head">
          <h2>Packs</h2>
          <button type="button" className="d-btn" onClick={() => setDialog({ pack: null })}>
            <Plus className="d-icon" aria-hidden />
            Add
          </button>
        </div>
        <label className="d-search glass">
          <Search className="d-icon" aria-hidden />
          <input
            type="search"
            aria-label="Search packs"
            placeholder="Search name, code, tag"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
          />
        </label>
        <div className="d-day">
          <Select label="Sort packs" value={sort} options={SORTS} onChange={setSort} />
        </div>
        {list.data === undefined && <p className="d-empty">{list.error ?? "Loading…"}</p>}
        {list.data && visible.length === 0 && <p className="d-empty">{query.trim() ? "No pack matches." : "No packs yet."}</p>}
        {visible.map(lane)}
        {hidden.length > 0 && (
          <>
            <div className="d-day">
              <button type="button" aria-expanded={showHidden} onClick={() => setShowHidden(!showHidden)}>
                {`Hidden · ${hidden.length} ${showHidden ? "▾" : "▸"}`}
              </button>
            </div>
            {showHidden && hidden.map(lane)}
          </>
        )}
      </section>
      <main className="d-main">
        {selectedId == null ? (
          <p className="d-empty">Add a pack to start.</p>
        ) : (
          <PackView
            key={selectedId}
            {...props}
            id={selectedId}
            edits={edits}
            onEdit={(pack) => setDialog({ pack })}
            changed={changed}
          />
        )}
      </main>
      {dialog && (
        <PackDialog
          api={api}
          notify={notify}
          pack={dialog.pack}
          onSaved={(id) => {
            setSelected(id);
            changed();
          }}
          onOpen={(id) => {
            setShowHidden(true);
            setSelected(id);
          }}
          onClose={() => setDialog(null)}
        />
      )}
    </>
  );
}
