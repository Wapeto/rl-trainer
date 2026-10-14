import { Filter, Search } from "lucide-react";
import { dayLabel } from "@shared/format";
import type { SessionCard } from "@shared/generated/types";
import { TERMS } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { HISTORY_SIZE, SOON_GONE_PLACE } from "@shared/review/history";
import { duration, span } from "../format";
import { savePhrase } from "../Sidebar";
import { cardSummary } from "./timeline";

interface SessionListProps {
  cards: SessionCard[] | null;
  selectedId: number | null;
  onSelect: (id: number) => void;
  now: number;
  query: string;
  onQuery: (query: string) => void;
  more: boolean;
  onMore: () => void;
  /** Ranked replays still to save, and the oldest one's place in match history. */
  toSave: { count: number; oldestPlace: number } | null;
  filtered: boolean;
  onFilter: () => void;
}

function byDay(cards: SessionCard[], now: number): { day: string; cards: SessionCard[] }[] {
  return cards.reduce<{ day: string; cards: SessionCard[] }[]>((groups, card) => {
    const day = dayLabel(card.started_at, now);
    const last = groups[groups.length - 1];
    if (last?.day === day) return [...groups.slice(0, -1), { day, cards: [...last.cards, card] }];
    return [...groups, { day, cards: [card] }];
  }, []);
}

export function SessionList(props: SessionListProps) {
  const { cards, selectedId, onSelect, now, query, onQuery, more, onMore, toSave, filtered, onFilter } = props;
  return (
    <section className="d-list" aria-label="Sessions">
      <div className="d-list__head">
        <h2>Sessions</h2>
      </div>
      <label className="d-search glass">
        <Search className="d-icon" aria-hidden />
        <input
          type="search"
          aria-label="Search sessions"
          placeholder="Search packs, codes, notes"
          value={query}
          onChange={(e) => onQuery(e.target.value)}
        />
      </label>
      {toSave && (
        <Hint gloss={TERMS.to_save} control className="hint--block">
          <button type="button" className="d-lane d-lane--save" aria-pressed={filtered} onClick={onFilter}>
            <span className="d-lane__t">{savePhrase(toSave.count)}</span>
            <Filter className="d-icon d-lane__f" aria-hidden />
            <span className={`d-pill${toSave.oldestPlace >= SOON_GONE_PLACE ? " d-pill--soon" : ""}`}>
              {`oldest #${toSave.oldestPlace} of ${HISTORY_SIZE}`}
            </span>
          </button>
        </Hint>
      )}
      {cards === null && <p className="d-empty">Loading…</p>}
      {cards?.length === 0 && (
        <p className="d-empty">{query.trim() ? "No session matches." : "No sessions yet. Play a bit and they show up here."}</p>
      )}
      {byDay(cards ?? [], now).map((group) => (
        <div key={group.day}>
          <div className="d-day">{group.day}</div>
          {group.cards.map((card) => {
            const on = card.id === selectedId;
            const length = duration(card.started_at, card.ended_at);
            return (
              <button
                key={card.id}
                type="button"
                className={`d-lane${on ? " d-lane--on" : ""}`}
                aria-current={on ? "true" : undefined}
                onClick={() => onSelect(card.id)}
              >
                <span className="d-lane__t">{span(card.started_at, card.ended_at)}</span>
                <span className="d-lane__r">{length ?? <span className="d-now">NOW</span>}</span>
                <span className="d-lane__sub">{cardSummary(card)}</span>
                <span className="d-lane__r">{card.needs_review && <span className="d-tag">Review</span>}</span>
              </button>
            );
          })}
        </div>
      ))}
      {more && (
        <button type="button" className="d-btn d-more" onClick={onMore}>
          Show more
        </button>
      )}
    </section>
  );
}
