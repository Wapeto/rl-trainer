import { useCallback, useEffect, useState } from "react";
import type { SessionCard } from "@shared/generated/types";
import { sentence } from "../act";
import { errorText } from "../api";
import { useDebounced, useLoad } from "../components/useLoad";
import type { PageProps } from "../pageProps";
import { GetStarted } from "./GetStarted";
import { SessionList } from "./SessionList";
import { SessionView } from "./SessionView";

export const PAGE_SIZE = 50;
/** How far the list loads on by itself to show a session opened from elsewhere. */
const MAX_FOCUS_PAGES = 20;

export function SessionsPage(props: PageProps) {
  const { api, now, reload, notify, onChanged } = props;
  const [query, setQuery] = useState("");
  const search = useDebounced(query, 250);
  const [cards, setCards] = useState<SessionCard[] | null>(null);
  const [more, setMore] = useState(false);
  const [selected, setSelected] = useState<number | null>(() => props.focusId ?? null);
  const [listTick, setListTick] = useState(0);
  const [notice, setNotice] = useState<string | null>(null);

  // Pages shown for the current search; a reload keeps them, a new search starts over.
  const [shown, setShown] = useState({ search: "", pages: 1 });
  const pages = shown.search === search ? shown.pages : 1;
  const toSave = useLoad(() => api.replaysToSave(), reload).data ?? [];
  const [filtered, setFiltered] = useState(false);
  // The filter turns itself off once nothing is left to save.
  const filterOn = filtered && toSave.length > 0;

  useEffect(() => {
    let current = true;
    const limit = pages * PAGE_SIZE;
    api.sessions(search, 0, limit, filterOn).then(
      (list) => {
        if (!current) return;
        setCards(list);
        setMore(list.length === limit);
      },
      (err: unknown) => current && notify(sentence(errorText(err)), "error"),
    );
    return () => {
      current = false;
    };
  }, [api, search, pages, reload, listTick, notify, filterOn]);

  // A session opened from Progress may be older than the loaded pages: load on until it shows.
  useEffect(() => {
    // A full list for the current pages: older cards still on screen would load on again and again.
    if (selected == null || cards?.length !== pages * PAGE_SIZE || pages >= MAX_FOCUS_PAGES) return;
    if (!cards.some((c) => c.id === selected)) setShown({ search, pages: pages + 1 });
  }, [selected, cards, pages, search]);

  function showMore() {
    setShown({ search, pages: pages + 1 });
  }

  const gone = useCallback((id: number, vanished = false) => {
    // Out of the list at once: until the reload answers, the newest card would otherwise
    // still be the removed session, and asking for it again would say it vanished.
    setCards((list) => list?.filter((card) => card.id !== id) ?? null);
    setSelected(null);
    setNotice(vanished ? "This session no longer exists." : null);
    setListTick((n) => n + 1);
    onChanged();
  }, [onChanged]);

  const select = useCallback((id: number) => {
    setNotice(null);
    setSelected(id);
    setListTick((n) => n + 1);
  }, []);

  const selectedId = selected ?? cards?.[0]?.id ?? null;
  return (
    <>
      <SessionList
        cards={cards}
        selectedId={selectedId}
        onSelect={(id) => {
          setNotice(null);
          setSelected(id);
        }}
        now={now}
        query={query}
        onQuery={setQuery}
        more={more}
        onMore={showMore}
        toSave={toSave.length ? { count: toSave.length, oldestPlace: Math.max(...toSave.map((m) => m.place)) } : null}
        filtered={filterOn}
        onFilter={() => setFiltered((f) => !f)}
      />
      <main className="d-main">
        <GetStarted {...props} />
        {notice && (
          <p className="d-err" role="status">
            {notice}
          </p>
        )}
        {selectedId == null ? (
          <p className="d-empty">{cards === null ? "Loading…" : "Pick a session on the left."}</p>
        ) : (
          <SessionView key={selectedId} {...props} id={selectedId} onGone={gone} onSelect={select} />
        )}
      </main>
    </>
  );
}
