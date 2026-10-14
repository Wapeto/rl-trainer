import { useState, type ReactNode } from "react";
import { CircleCheck, CircleDashed } from "lucide-react";
import { act } from "../act";
import { useLoad } from "../components/useLoad";
import type { PageProps } from "../pageProps";
import { accountText, statsText } from "../settings/sections";

interface StepProps {
  done: boolean;
  title: string;
  text: string;
  action?: ReactNode;
}

function Step({ done, title, text, action }: StepProps) {
  const Icon = done ? CircleCheck : CircleDashed;
  return (
    <li className={`d-srow d-step${done ? " d-step--done" : ""}`}>
      <Icon className="d-icon" aria-label={done ? "Done" : "To do"} />
      <div className="d-srow__tx">
        <b>{title}</b>
        <span>{text}</span>
      </div>
      {!done && action}
    </li>
  );
}

/** Three steps for a new install; hidden once the first two are done or when dismissed. */
export function GetStarted({ api, snapshot, notify, reload, onChanged, onOpenSettings }: PageProps) {
  const [tick, setTick] = useState(0);
  const key = `${reload}|${tick}`;
  const settings = useLoad(() => api.settings(), key).data;
  const profiles = useLoad(() => api.profiles(), key).data;
  if (!snapshot || !settings || !profiles || settings.setup_dismissed) return null;

  const stats = statsText(snapshot.stats_api);
  const account = profiles.some((p) => p.is_active);
  if (stats.ok && account) return null;
  const phone = snapshot.phone_clients > 0;

  async function run(action: () => Promise<unknown>, okText: string) {
    if (await act(notify, action, okText)) {
      setTick((n) => n + 1);
      onChanged();
    }
  }
  const openSettings = onOpenSettings && (
    <button type="button" className="d-btn" onClick={onOpenSettings}>
      Open Settings
    </button>
  );
  const statsAction = stats.fix ? (
    <button type="button" className="d-btn d-btn--warn" onClick={() => run(() => api.fixStatsApi(), "Stats API turned on")}>
      Fix…
    </button>
  ) : (
    openSettings
  );
  return (
    <section className="d-sec d-start" aria-label="Get started">
      <div className="d-start__hd">
        <h2 className="d-k">Get started</h2>
        <button type="button" className="d-link" onClick={() => run(() => api.updateSettings({ setup_dismissed: true }), "Hidden")}>
          Dismiss
        </button>
      </div>
      <ol className="d-card glass">
        <Step
          done={stats.ok}
          title="Turn on the Stats API"
          text={stats.text.replace("set its folder below", "set its folder in Settings")}
          action={statsAction}
        />
        <Step
          done={account}
          title="Your account"
          text={account ? accountText(profiles) : "Open free play, or enter your in-game name in Settings."}
          action={openSettings}
        />
        <Step
          done={phone}
          title="Phone (optional)"
          text={settings.phone_enabled ? "Scan the QR code in Settings with your phone." : "Log reps and see your stats on your phone over Wi-Fi."}
          action={
            settings.phone_enabled ? (
              openSettings
            ) : (
              <button type="button" className="d-btn" onClick={() => run(() => api.enablePhone(), "Phone page on")}>
                Turn on phone page
              </button>
            )
          }
        />
      </ol>
    </section>
  );
}
