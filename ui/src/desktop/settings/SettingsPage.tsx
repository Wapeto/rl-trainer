import { useState, type KeyboardEvent } from "react";
import { sentence } from "../act";
import { errorText, type SettingsPatch } from "../api";
import { useLoad } from "../components/useLoad";
import type { PageProps } from "../pageProps";
import { AboutSection, AppSection, GameSection, PhoneSection, ReplaysSection, TrackingSection, statsText } from "./sections";

export type SettingsTab = "game" | "phone" | "replays" | "account" | "app" | "about";
type Tab = SettingsTab;

const TABS: { id: Tab; label: string; lead: string }[] = [
  { id: "game", label: "Game", lead: "How RL Trainer reads Rocket League. Restart the game after turning the Stats API on." },
  { id: "phone", label: "Phone", lead: "Log reps and see your stats on your phone, on the same Wi-Fi." },
  { id: "replays", label: "Replays", lead: "Saved replays add detailed stats to your matches, and can bring in games played before." },
  { id: "account", label: "Account & tracking", lead: "Which player is you, and how sessions are counted." },
  { id: "app", label: "App", lead: "Starting with Windows, the install count, quitting and backups." },
  { id: "about", label: "About", lead: "Version, updates and where your data lives." },
];

/** The tab last open, for this run of the app only. */
let lastTab: Tab | null = null;

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

  // Topics that need the user: the Stats API file is not right, or no account is tracked.
  const attention: Partial<Record<Tab, boolean>> = {
    game: snapshot != null && !statsText(snapshot.stats_api).ok,
    account: profiles.data != null && !profiles.data.some((p) => p.is_active),
  };
  const [picked, setPicked] = useState<Tab | null>(lastTab);
  const tab = picked ?? TABS.find((t) => attention[t.id])?.id ?? "game";
  const choose = (t: Tab) => {
    lastTab = t;
    setPicked(t);
  };

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

  function onTabKey(e: KeyboardEvent<HTMLDivElement>) {
    const i = TABS.findIndex((t) => t.id === tab);
    const next = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: TABS.length - 1 }[e.key];
    if (next == null) return;
    e.preventDefault();
    const t = TABS[(next + TABS.length) % TABS.length].id;
    choose(t);
    document.getElementById(`settings-tab-${t}`)?.focus();
  }

  const s = settings.data;
  const current = TABS.find((t) => t.id === tab)!;
  return (
    <main className="d-main">
      <div className="d-settings">
        <div className="d-hd">
          <h1>Settings</h1>
        </div>
        {!s ? (
          <p className="d-empty">{settings.error ?? "Loading…"}</p>
        ) : (
          <div className="d-stabs">
            <div className="d-stabs__list" role="tablist" aria-orientation="vertical" aria-label="Settings" onKeyDown={onTabKey}>
              {TABS.map((t) => (
                <button
                  key={t.id}
                  id={`settings-tab-${t.id}`}
                  type="button"
                  role="tab"
                  aria-selected={t.id === tab}
                  aria-controls="settings-panel"
                  tabIndex={t.id === tab ? 0 : -1}
                  className={`d-stabs__tab${t.id === tab ? " d-stabs__tab--on" : ""}`}
                  onClick={() => choose(t.id)}
                >
                  {t.label}
                  {attention[t.id] && (
                    <>
                      <span className="d-stabs__dot" aria-hidden="true" />
                      <span className="d-sr">needs attention</span>
                    </>
                  )}
                </button>
              ))}
            </div>
            <div className="d-stabs__panel" role="tabpanel" id="settings-panel" aria-labelledby={`settings-tab-${tab}`}>
              <h2>{current.label}</h2>
              <p className="d-stabs__lead">{current.lead}</p>
              {tab === "game" && (
                <GameSection api={api} notify={notify} settings={s} status={snapshot?.stats_api} live={snapshot ?? undefined} save={save} />
              )}
              {tab === "phone" && (
                <PhoneSection
                  api={api}
                  notify={notify}
                  settings={s}
                  pairing={pairing.data ?? null}
                  phones={snapshot?.phone_clients ?? 0}
                  refresh={refresh}
                  save={save}
                />
              )}
              {tab === "replays" && (
                <ReplaysSection api={api} notify={notify} settings={s} status={replays.data} save={save} refresh={refresh} onChanged={onChanged} />
              )}
              {tab === "account" && <TrackingSection api={api} notify={notify} settings={s} profiles={profiles.data ?? []} refresh={refresh} save={save} />}
              {tab === "app" && (
                <AppSection
                  api={api}
                  notify={notify}
                  now={now}
                  settings={s}
                  save={save}
                  autostart={autostart.data}
                  backups={backups.data ?? []}
                  about={about.data}
                  refresh={refresh}
                  onRestored={() => {
                    refresh();
                    onChanged();
                  }}
                />
              )}
              {tab === "about" && <AboutSection api={api} notify={notify} about={about.data} error={about.error} onCheckUpdate={onCheckUpdate} />}
            </div>
          </div>
        )}
      </div>
    </main>
  );
}

/** Opens Settings on this tab next time (the Get started links); null starts fresh. */
export function presetSettingsTab(t: SettingsTab | null) {
  lastTab = t;
}
