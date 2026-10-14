import { Sun } from "lucide-react";
import { useEffect, useSyncExternalStore } from "react";
import type { Api } from "@shared/api/client";
import type { LiveSource } from "@shared/api/liveStore";
import { Toast, useToast } from "@shared/components/Toast";
import { useNow } from "@shared/hooks/useNow";
import type { AwakeState } from "@shared/keepAwake";
import type { ScreenProps } from "./screenProps";
import { Connecting } from "./screens/Connecting";
import { Freeplay } from "./screens/Freeplay";
import { Idle } from "./screens/Idle";
import { Match } from "./screens/Match";
import { Pairing } from "./screens/Pairing";
import { SessionEnd } from "./screens/SessionEnd";
import { Training } from "./screens/Training";
import { reviewTarget, selectScreen, type Screen } from "./selectScreen";

function ScreenFor({ screen, props }: { screen: Screen; props: ScreenProps }) {
  switch (screen) {
    case "training":
      return <Training {...props} />;
    case "freeplay":
      return <Freeplay {...props} />;
    case "match":
      return <Match {...props} />;
    case "session_end":
      // A new session to review gets a fresh form, never the previous one's fields.
      return <SessionEnd key={reviewTarget(props.snap)?.id} {...props} />;
    default:
      return <Idle {...props} />;
  }
}

export interface KeepAwakeControl {
  getState(): AwakeState;
  subscribe(listener: () => void): () => void;
  enable(): Promise<void>;
}

const noSubscribe = () => () => {};
const alwaysOff = (): AwakeState => "off";

/** Keeps the screen on from the first tap: browsers only allow it after one. `click` (not a
 * touch `pointerdown`) is what counts as that tap on phones. */
function useKeepAwake(keepAwake: KeepAwakeControl | undefined): AwakeState {
  const awake = useSyncExternalStore(keepAwake?.subscribe ?? noSubscribe, keepAwake?.getState ?? alwaysOff);
  useEffect(() => {
    if (!keepAwake) return;
    const onTap = () => void keepAwake.enable();
    document.addEventListener("click", onTap);
    return () => document.removeEventListener("click", onTap);
  }, [keepAwake]);
  return awake;
}

interface PhoneAppProps {
  source: LiveSource;
  api: Api | null;
  /** Fixed time for tests; the live clock otherwise. */
  now?: number;
  keepAwake?: KeepAwakeControl;
}

export function PhoneApp({ source, api, now: fixedNow, keepAwake }: PhoneAppProps) {
  const state = useSyncExternalStore(source.subscribe, source.getState);
  const awake = useKeepAwake(keepAwake);
  const ticking = useNow();
  const now = fixedNow ?? ticking;
  const toast = useToast();
  const screen = selectScreen(state, now);
  return (
    <main className={`phone${state.conn === "reconnecting" ? " phone--dim" : ""}`} data-screen={screen}>
      {screen === "pairing" && <Pairing />}
      {screen === "connecting" && <Connecting />}
      {state.snapshot && screen !== "pairing" && screen !== "connecting" && (
        <ScreenFor
          screen={screen}
          props={{ snap: state.snapshot, conn: state.conn, now, api, notify: toast.show }}
        />
      )}
      {keepAwake && awake === "off" && screen !== "pairing" && (
        <button type="button" className="awake-hint pill glass">
          <Sun size={14} aria-hidden />
          Tap to keep the screen on
        </button>
      )}
      <Toast toast={toast.current} />
    </main>
  );
}
