import type { ProgressRange } from "@shared/generated/types";
import { useState } from "react";
import { useLoad } from "../components/useLoad";
import { readKey, writeKey, type KeyValueStore } from "../components/storage";
import type { PageProps } from "../pageProps";
import { emptyRangeText } from "./format";
import { ModeView } from "./ModeView";
import { OverviewView } from "./OverviewView";
import { PackProgressView } from "./PackProgressView";
import { ProgressList } from "./ProgressList";
import "./progress.css";

export type Selection = { kind: "overview" } | { kind: "mode"; teamSize: number } | { kind: "pack"; id: number };

export interface ProgressPageProps extends PageProps {
  storage: KeyValueStore | null;
  onOpenSession: (id: number) => void;
  onOpenPack: (id: number) => void;
}

const RANGE_KEY = "rl-progress-range";
const RANGES: ProgressRange[] = ["days30", "days90", "all"];

function readRange(storage: KeyValueStore | null): ProgressRange {
  const saved = readKey(storage, RANGE_KEY);
  return RANGES.find((r) => r === saved) ?? "days90";
}

export function ProgressPage(props: ProgressPageProps) {
  const { api, reload, storage } = props;
  const [range, setRange] = useState<ProgressRange>(() => readRange(storage));
  const [selection, setSelection] = useState<Selection>({ kind: "overview" });
  const list = useLoad(() => api.progressList(range), `${range}|${reload}`);

  function changeRange(next: ProgressRange) {
    setRange(next);
    writeKey(storage, RANGE_KEY, next);
  }

  const empty = list.data != null && list.data.sessions === 0;
  return (
    <>
      <ProgressList data={list.data} range={range} onRange={changeRange} selection={selection} onSelect={setSelection} />
      <main className="d-main">
        {list.error && (
          <p className="d-err" role="alert">
            {list.error}
          </p>
        )}
        {empty ? (
          <div className="d-empty">
            <p>{emptyRangeText(range)}</p>
            {range !== "all" && (
              <button type="button" className="d-btn" onClick={() => changeRange("all")}>
                Show all
              </button>
            )}
          </div>
        ) : selection.kind === "overview" ? (
          <OverviewView {...props} range={range} onSelect={setSelection} />
        ) : selection.kind === "mode" ? (
          <ModeView {...props} key={selection.teamSize} range={range} teamSize={selection.teamSize} />
        ) : (
          <PackProgressView {...props} key={selection.id} range={range} packId={selection.id} />
        )}
      </main>
    </>
  );
}
