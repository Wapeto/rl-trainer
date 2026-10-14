import type { KeyboardEvent } from "react";
import type { Gloss } from "@shared/glossary";
import { Hint } from "@shared/hint/Hint";
import { LineChart, type LineChartProps } from "./LineChart";

export type ChartCardChart = Omit<LineChartProps, "label" | "height" | "compact">;

interface ChartCardProps {
  title: string;
  /** The value over the range, shown in the corner. */
  value: string;
  note?: string | null;
  chart: ChartCardChart;
  height?: number;
  compact?: boolean;
  /** Makes the card a toggle that chooses this stat (small multiples). */
  onSelect?: () => void;
  selected?: boolean;
  /** What the chart shows: on the title, or on the whole card when it can be chosen. */
  gloss?: Gloss | null;
}

/** A titled chart. With `onSelect`, clicking the card (anywhere but on a point) chooses it. */
export function ChartCard(props: ChartCardProps) {
  const { title, value, note, chart, height = 92, compact = true, onSelect, selected = false, gloss = null } = props;
  const body = (
    <>
      <div className="c-card__head">
        <span className="c-card__t">{onSelect ? title : <Hint gloss={gloss}>{title}</Hint>}</span>
        <span className="c-card__v">{value}</span>
      </div>
      {note && <p className="c-card__note">{note}</p>}
      <LineChart {...chart} label={title} height={height} compact={compact} />
    </>
  );
  if (!onSelect) {
    return (
      <section className="c-card glass" aria-label={title}>
        {body}
      </section>
    );
  }
  const onKey = (e: KeyboardEvent) => {
    if (e.target === e.currentTarget && (e.key === "Enter" || e.key === " ")) {
      e.preventDefault();
      onSelect();
    }
  };
  // A div with the button role: the chart inside has its own clickable points.
  return (
    <Hint gloss={gloss} control className="hint--block">
      <div
        className={`c-card glass c-card--pick${selected ? " c-card--on" : ""}`}
        role="button"
        tabIndex={0}
        aria-label={title}
        aria-pressed={selected}
        onClick={onSelect}
        onKeyDown={onKey}
      >
        {body}
      </div>
    </Hint>
  );
}
