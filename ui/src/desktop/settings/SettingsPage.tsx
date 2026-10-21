import { useState } from "react";
import { sentence } from "../act";
import { errorText, type SettingsPatch } from "../api";
import { useLoad } from "../components/useLoad";
import type { PageProps } from "../pageProps";
import { AppSection, GameSection, PhoneSection, ReplaysSection, TrackingSection } from "./sections";

export function SettingsPage({ api, snapshot, now, notify, reload, onChanged, onCheckUpdate }: PageProps) {
  const [tick, setTick] = useState(0);
  const key = `${reload}|${tick}`;
  const settings = useLoad(() => api.settings(), key);
  const pairing = useLoad(() => api.pairing(), key);
  const profiles = useLoad(() => api.profiles(), key);
  const replays = useLoad(() => api.replayStatus(), key);
  const backups = useLoad(() => api.backups(), key);
  const autostart = useLoad(() => api.autostart(), key);
  const about = useLoad(() => api.about(), "about");
  const refresh = () => setTick((n) => n + 1);

  async function save(patch: SettingsPatch, okText: string): Promise<string | null> {
    try {
      await api.updateSettings(patch);
      notify(okText, "ok");
      refresh();
      return null;
    } catch (err) {
      return sentence(errorText(err));
    }
  }

  const s = settings.data;
  return (
    <main className="d-main">
      <div className="d-settings">
        <div className="d-hd">
          <h1>Settings</h1>
        </div>
        {!s ? (
          <p className="d-empty">{settings.error ?? "Loading…"}</p>
        ) : (
          <>
            <PhoneSection
              api={api}
              notify={notify}
              settings={s}
              pairing={pairing.data ?? null}
              phones={snapshot?.phone_clients ?? 0}
              refresh={refresh}
              save={save}
            />
            <GameSection api={api} notify={notify} settings={s} status={snapshot?.stats_api} live={snapshot ?? undefined} save={save} />
            <ReplaysSection api={api} notify={notify} settings={s} status={replays.data} save={save} refresh={refresh} />
            <TrackingSection api={api} notify={notify} settings={s} profiles={profiles.data ?? []} refresh={refresh} save={save} />
            <AppSection
              api={api}
              notify={notify}
              now={now}
              autostart={autostart.data}
              backups={backups.data ?? []}
              about={about.data}
              refresh={refresh}
              onCheckUpdate={onCheckUpdate}
              onRestored={() => {
                refresh();
                onChanged();
              }}
            />
          </>
        )}
      </div>
    </main>
  );
}
