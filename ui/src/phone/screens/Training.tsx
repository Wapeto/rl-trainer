import { useState } from "react";
import { OutcomeDots } from "@shared/components/OutcomeDots";
import { Ring, type Tone } from "@shared/components/Ring";
import { Button, Pill, StatRow } from "@shared/components/ui";
import { elapsed, ratePercent } from "@shared/format";
import type { LiveTraining } from "@shared/generated/types";
import { act } from "../act";
import { Header, sessionPill } from "../Header";
import { NudgeBanner } from "../NudgeBanner";
import type { ScreenProps } from "../screenProps";
import { PackPicker } from "./PackPicker";

/** Target used while no pack is set (the pack table's default). */
export const DEFAULT_TARGET = 0.65;

const EMPTY: LiveTraining = { attempts: 0, makes: 0, make_rate: null, streak: 0, recent: [], nudge: null };

export function targetChip(rate: number | null, target: number): string {
  const t = Math.round(target * 100);
  if (rate == null) return `Target ${t}%`;
  const diff = Math.round(rate * 100) - t;
  if (diff === 0) return `On target ${t}%`;
  return diff > 0 ? `+${diff} above target ${t}%` : `−${-diff} vs target ${t}%`;
}

export function Training({ snap, conn, now, api, notify }: ScreenProps) {
  const [picking, setPicking] = useState(false);
  const block = snap.live.block!;
  const t = block.training ?? EMPTY;
  const target = block.pack?.target_rate ?? DEFAULT_TARGET;
  const rate = t.make_rate;
  const tone: Tone =
    rate == null ? "neutral" : rate >= target ? "green" : "amber";
  const last = t.recent.at(-1) ?? null;
  const suggestion = block.pack ? null : block.suggested_pack;
  const percent = ratePercent(rate);
  return (
    <div className={`screen tone-${tone}`}>
      <NudgeBanner nudge={t.nudge} />
      <Header
        conn={conn}
        left={<Pill live>Training</Pill>}
        right={sessionPill(snap, now)}
      />
      <h1 className="title">{block.pack?.name ?? "No pack set"}</h1>
      {block.pack && <p className="subtitle">{block.pack.code}</p>}
      <div className="hero">
        <Ring
          size="hero"
          tone={tone}
          value={rate ?? 0}
          target={target}
          label={`Make rate ${rate == null ? "none yet" : `${percent}%`}, target ${Math.round(target * 100)}%`}
        >
          <div className="hero__number">
            {percent}
            {rate != null && <small>%</small>}
          </div>
          <span className="chip glass">{targetChip(rate, target)}</span>
        </Ring>
      </div>
      <div className="dots-wrap">
        <OutcomeDots attempts={t.recent} />
        <div className="legend">
          <span>Last 10</span>
          <span>newest →</span>
        </div>
      </div>
      <StatRow
        items={[
          { label: "Made", value: `${t.makes}/${t.attempts}` },
          { label: "On this pack", value: elapsed(now - block.started_at) },
          { label: "Streak", value: t.streak > 0 ? `${t.streak} ✓` : "—" },
        ]}
      />
      <div className="actions">
        {suggestion && (
          <Button variant="text" onClick={() => setPicking(true)}>
            Change pack
          </Button>
        )}
        <div className="btns">
          <Button
            disabled={!last || !api}
            onClick={() =>
              last &&
              api &&
              act(
                notify,
                () => api.setSkipped(last.id, !last.skipped),
                last.skipped ? "Skip undone" : "Skipped",
              )
            }
          >
            {last?.skipped ? "Undo skip" : "Skip last"}
          </Button>
          {suggestion ? (
            <Button
              variant="primary"
              disabled={!api}
              onClick={() =>
                api &&
                act(
                  notify,
                  () => api.setPack(block.id, suggestion.id),
                  "Pack set",
                )
              }
            >
              Still {suggestion.name}?
            </Button>
          ) : (
            <Button
              variant="primary"
              disabled={!api}
              onClick={() => setPicking(true)}
            >
              Change pack
            </Button>
          )}
        </div>
      </div>
      <PackPicker
        open={picking}
        blockId={block.id}
        currentPackId={block.pack?.id ?? null}
        suggestion={suggestion}
        api={api}
        notify={notify}
        onClose={() => setPicking(false)}
      />
    </div>
  );
}
