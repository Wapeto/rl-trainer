import { Copy } from "lucide-react";
import { useState } from "react";
import type { Notify } from "@shared/components/Toast";
import { dayLabel, timeOfDay } from "@shared/format";
import type {
  About,
  AppSnapshot,
  BackupInfo,
  PairingInfo,
  ProfileRow,
  ReplayStatus,
  Settings,
  StatsApiStatus,
} from "@shared/generated/types";
import { act } from "../act";
import type { DesktopApi, SettingsPatch } from "../api";
import { Confirm } from "../components/Dialog";
import { plural } from "../format";
import { EditSetting, Row, Switch } from "./fields";
import { Select } from "../components/Select";

/** Saves a settings change; returns the error text for the field, or null. */
export type SaveSetting = (patch: SettingsPatch, okText: string) => Promise<string | null>;

export const RELEASES_URL = "https://github.com/Wapeto/rl-trainer/releases";

export const maskToken = (url: string) => url.replace(/#t=.*/, "#t=••••••••");

const whole = (raw: string) => (/^\d+$/.test(raw) ? Number(raw) : null);
const number = (raw: string) => (raw !== "" && Number.isFinite(Number(raw)) ? Number(raw) : null);

/** The game reads its settings file only at launch, so a fixed file isn't enough while it runs. */
export const FIXED_TEXT = "Stats API turned on. Restart Rocket League if it's open.";
const WAITING = "Waiting for data. If this stays, restart Rocket League.";

type Live = Pick<AppSnapshot, "game_running" | "ingest_connected">;

export function statsText(s: StatsApiStatus | undefined, live?: Live): { ok: boolean; text: string; fix: boolean } {
  switch (s?.state) {
    case "ok":
      if (live?.game_running && !live.ingest_connected) return { ok: false, text: WAITING, fix: false };
      return { ok: true, text: "Working", fix: false };
    case "disabled":
      return { ok: false, text: "Turned off in the game's settings file", fix: true };
    case "file_missing":
      return { ok: false, text: "The game's settings file is missing", fix: true };
    case "unreadable":
      return { ok: false, text: "Can't read the game's settings file", fix: true };
    case "install_not_found":
      return { ok: false, text: "Rocket League not found: set its folder below", fix: false };
    default:
      return { ok: false, text: "Not checked yet", fix: false };
  }
}

export function backupLabel(b: BackupInfo): string {
  switch (b.kind) {
    case "daily":
      return "daily";
    case "pre_migration":
      return "before an update";
    case "pre_restore":
      return "before a restore";
    case "pre_edit": {
      const action = b.name.match(/-(merge|split|delete)\.db$/);
      return action ? `before ${action[1]}` : "before an edit";
    }
  }
}

interface PhoneProps {
  api: DesktopApi;
  notify: Notify;
  settings: Settings;
  pairing: PairingInfo | null;
  phones: number;
  refresh: () => void;
  save: SaveSetting;
}

export function PhoneSection({ api, notify, settings, pairing, phones, refresh, save }: PhoneProps) {
  async function toggle(on: boolean) {
    const ok = await act(notify, () => (on ? api.enablePhone() : api.disablePhone()), on ? "Phone page on" : "Phone page off");
    if (ok) refresh();
  }
  async function newCode() {
    if (await act(notify, () => api.regenerateToken(), "New pairing code made")) refresh();
  }
  async function copy(url: string) {
    try {
      await navigator.clipboard.writeText(url);
      notify("Link copied", "ok");
    } catch {
      notify("Couldn't copy the link", "error");
    }
  }
  return (
    <section className="d-sec" aria-label="Phone">
      <span className="d-k">Phone</span>
      <div className="d-card glass">
        <Row
          title="Phone page"
          hint="Lets your phone open the tracker on this Wi-Fi. Windows must treat this Wi-Fi as Private: Settings → Network & internet → your network → Private."
        >
          <Switch label="Phone page" on={settings.phone_enabled} onChange={toggle} />
        </Row>
        {settings.phone_enabled && pairing && (
          <div className="d-srow d-srow--top">
            <img
              className="d-qr"
              alt="QR code to pair your phone"
              src={`data:image/svg+xml;utf8,${encodeURIComponent(pairing.qr_svg)}`}
            />
            <div className="d-srow__tx">
              <b>Scan with your phone</b>
              <span>Or open this address in its browser:</span>
              <div className="d-url">{maskToken(pairing.url)}</div>
              <div className="d-btns d-btns--gap">
                <button type="button" className="d-btn" onClick={() => copy(pairing.url)}>
                  <Copy className="d-icon" aria-hidden />
                  Copy link
                </button>
                <button type="button" className="d-btn" onClick={newCode}>
                  New pairing code
                </button>
              </div>
              <span className="d-hint">A new code signs out the phones that use the old one.</span>
            </div>
            <span className={phones > 0 ? "d-ok" : "d-warn"}>
              {phones > 0 ? `${plural(phones, "phone")} connected` : "No phone connected"}
            </span>
          </div>
        )}
        <Row title="Port" hint="Change only if another app uses it.">
          <EditSetting
            label="Port"
            value={String(settings.phone_port)}
            numeric
            onSave={(raw) => {
              const port = whole(raw);
              return port == null ? Promise.resolve("Enter a whole number") : save({ phone_port: port }, "Port saved");
            }}
          />
        </Row>
      </div>
    </section>
  );
}

interface GameProps {
  api: DesktopApi;
  notify: Notify;
  settings: Settings;
  status: StatsApiStatus | undefined;
  live?: Live;
  save: SaveSetting;
}

export function GameSection({ api, notify, settings, status, live, save }: GameProps) {
  const s = statsText(status, live);
  const waiting = status?.state === "ok" && !s.ok;
  return (
    <section className="d-sec" aria-label="Game">
      <span className="d-k">Game</span>
      <div className="d-card glass">
        <Row title="Stats API" hint="Rocket League sends live data to the app.">
          <span className={s.ok ? "d-ok" : "d-warn"}>{s.text}</span>
          {s.fix && (
            <button type="button" className="d-btn d-btn--warn" onClick={() => act(notify, () => api.fixStatsApi(), FIXED_TEXT)}>
              Fix…
            </button>
          )}
          {!s.ok && !s.fix && !waiting && (
            <button type="button" className="d-btn" onClick={() => act(notify, () => api.recheckStatsApi())}>
              Check again
            </button>
          )}
        </Row>
        <Row
          title="Rocket League folder"
          hint={settings.install_dir ? "Set by you. Empty it to find the game automatically." : "Found automatically. Paste a folder to override."}
        >
          <EditSetting
            label="Rocket League folder"
            value={settings.install_dir ?? ""}
            wide
            placeholder="C:\Program Files\Epic Games\rocketleague"
            onSave={(raw) =>
              save({ install_dir: raw === "" ? null : raw }, raw === "" ? "The game will be found automatically" : "Folder saved")
            }
          />
        </Row>
      </div>
    </section>
  );
}

interface ReplaysProps {
  settings: Settings;
  status: ReplayStatus | undefined;
  save: SaveSetting;
}

/** The replay folder being watched and what was found in it (spec §4.1, §9). */
export function ReplaysSection({ settings, status, save }: ReplaysProps) {
  const c = status?.counts;
  const folders = status?.folders ?? [];
  return (
    <section className="d-sec" aria-label="Replays">
      <span className="d-k">Replays</span>
      <div className="d-card glass">
        <Row
          title="Replay folder"
          hint={
            folders.length
              ? `Watching ${folders.join(", ")}`
              : "No replay folder found. Paste the folder where the game saves replays."
          }
        >
          <EditSetting
            label="Replay folder"
            value={settings.replay_dir ?? ""}
            wide
            placeholder="…\My Games\Rocket League\TAGame\DemosEpic"
            onSave={(raw) =>
              save({ replay_dir: raw === "" ? null : raw }, raw === "" ? "The game's folder will be used" : "Folder saved")
            }
          />
        </Row>
        {c && (
          <Row title="Imported" hint={c.last_error ?? undefined}>
            <span className={c.format_warning ? "d-warn" : "d-ok"}>
              {c.format_warning
                ? "The last replays could not be read: the game may have changed its replay format. rl-trainer needs an update."
                : `${c.imported} imported${c.errors ? ` · ${c.errors} couldn't be read` : ""}`}
            </span>
          </Row>
        )}
      </div>
    </section>
  );
}

interface TrackingProps {
  api: DesktopApi;
  notify: Notify;
  settings: Settings;
  profiles: ProfileRow[];
  refresh: () => void;
  save: SaveSetting;
}

/** "Tracking Name (Epic)" or the nudge to play free play / type a name. */
export function accountText(profiles: ProfileRow[]): string {
  const me = profiles.find((p) => p.is_active);
  if (!me) return "Not detected yet: open free play, or enter your in-game name.";
  return `Tracking ${me.display_name} (${me.primary_id.split("|")[0]})`;
}

export function TrackingSection({ api, notify, settings, profiles, refresh, save }: TrackingProps) {
  const [restart, setRestart] = useState(false);
  const active = profiles.find((p) => p.is_active)?.primary_id ?? "";
  async function choose(primaryId: string) {
    if (await act(notify, () => api.setActiveProfile(primaryId), "Account changed")) {
      setRestart(true);
      refresh();
    }
  }
  async function saveHint(raw: string) {
    const problem = await save({ me_hint: raw }, "Account saved");
    if (!problem) setRestart(true);
    return problem;
  }
  return (
    <section className="d-sec" aria-label="Tracking">
      <span className="d-k">Tracking</span>
      <div className="d-card glass">
        <Row title="End a session after" hint="Minutes with the game open but nothing happening.">
          <EditSetting
            label="Session timeout"
            value={String(settings.session_timeout_min)}
            unit="min"
            numeric
            onSave={(raw) => {
              const min = whole(raw);
              return min == null ? Promise.resolve("Enter a whole number") : save({ session_timeout_min: min }, "Timeout saved");
            }}
          />
        </Row>
        <Row title="Hard hit from" hint="Free play counts hits at or above this speed.">
          <EditSetting
            label="Hard hit speed"
            value={String(settings.hard_hit_kmh)}
            unit="km/h"
            numeric
            onSave={(raw) => {
              const kmh = number(raw);
              return kmh == null ? Promise.resolve("Enter a number") : save({ hard_hit_kmh: kmh }, "Hard hit speed saved");
            }}
          />
        </Row>
        <Row title="Account" hint={`${accountText(profiles).replace(/\.?$/, ".")} Applies the next time the app starts.`}>
          {profiles.length > 1 && (
            <Select
              label="Account"
              value={active}
              options={profiles.map((p) => ({ value: p.primary_id, label: p.display_name }))}
              onChange={choose}
            />
          )}
          <EditSetting
            label="In-game name or Epic account ID"
            value={settings.me_hint ?? ""}
            wide
            placeholder="In-game name or Epic account ID"
            onSave={saveHint}
          />
          {restart && api.canControlApp && (
            <button type="button" className="d-btn d-btn--primary" onClick={() => act(notify, () => api.restartApp())}>
              Restart now
            </button>
          )}
        </Row>
      </div>
    </section>
  );
}

interface AppProps {
  api: DesktopApi;
  notify: Notify;
  now: number;
  autostart: boolean | null | undefined;
  backups: BackupInfo[];
  about: About | undefined;
  refresh: () => void;
  onCheckUpdate?: () => Promise<void>;
  onRestored: () => void;
}

export function AppSection({ api, notify, now, autostart, backups, about, refresh, onCheckUpdate, onRestored }: AppProps) {
  const [showAll, setShowAll] = useState(false);
  const [restoring, setRestoring] = useState<BackupInfo | null>(null);
  const [leaving, setLeaving] = useState<"quit" | "restart" | null>(null);
  const shown = showAll ? backups : backups.slice(0, 3);
  const when = (b: BackupInfo) => `${dayLabel(b.modified_ms, now)} ${timeOfDay(b.modified_ms)}`;

  async function setAutostart(on: boolean) {
    if (await act(notify, () => api.setAutostart(on))) refresh();
  }
  async function restore(b: BackupInfo) {
    if (await act(notify, () => api.restoreBackup(b.name), "Backup restored")) {
      setRestoring(null);
      onRestored();
    }
  }

  return (
    <section className="d-sec" aria-label="App">
      <span className="d-k">App</span>
      {autostart != null && (
        <div className="d-card glass">
          <Row title="Start with Windows" hint="Runs in the tray, ready when you launch the game.">
            <Switch label="Start with Windows" on={autostart} onChange={setAutostart} />
          </Row>
        </div>
      )}
      {api.canControlApp && (
        <div className="d-card glass">
          <Row title="RL Trainer" hint="Closing the window keeps it recording in the tray. Quit or restart it here.">
            <div className="d-btns">
              <button type="button" className="d-btn" onClick={() => setLeaving("restart")}>
                Restart
              </button>
              <button type="button" className="d-btn" onClick={() => setLeaving("quit")}>
                Quit
              </button>
            </div>
          </Row>
        </div>
      )}
      <div className="d-card glass">
        <Row title="Backups" hint="One a day (the last 7 are kept), plus one before each restore, merge, split or delete." />
        {backups.length === 0 && <p className="d-empty">No backup yet.</p>}
        {shown.map((b) => (
          <div key={b.name} className="d-bk">
            <span>
              {when(b)} <span className="d-m">· {backupLabel(b)}</span>
            </span>
            <span className="d-m">{(b.size_bytes / 1_000_000).toFixed(1)} MB</span>
            <button type="button" className="d-btn" onClick={() => setRestoring(b)}>
              Restore
            </button>
          </div>
        ))}
        {!showAll && backups.length > 3 && (
          <div className="d-bk">
            <button type="button" className="d-link" onClick={() => setShowAll(true)}>
              {`Show all (${backups.length})`}
            </button>
          </div>
        )}
      </div>
      {about && (
        <p className="d-foot">
          {`RL Trainer ${about.version} · Data in ${about.data_dir}`}
          <button type="button" className="d-link" onClick={() => act(notify, () => api.openDataFolder())}>
            Open folder
          </button>
          {onCheckUpdate && (
            <button type="button" className="d-link" onClick={onCheckUpdate}>
              Check for updates
            </button>
          )}
          <button type="button" className="d-link" title={RELEASES_URL} onClick={() => act(notify, () => api.openReleases())}>
            Releases
          </button>
        </p>
      )}
      {leaving === "quit" && (
        <Confirm
          title="Quit RL Trainer?"
          body={<p>This closes the session in progress and stops recording. You can start RL Trainer again from the Start menu.</p>}
          confirm="Quit"
          onConfirm={() => act(notify, () => api.quitApp())}
          onClose={() => setLeaving(null)}
        />
      )}
      {leaving === "restart" && (
        <Confirm
          title="Restart RL Trainer?"
          body={<p>This closes the session in progress, then opens RL Trainer again.</p>}
          confirm="Restart"
          busyLabel="Restarting…"
          onConfirm={() => act(notify, () => api.restartApp())}
          onClose={() => setLeaving(null)}
        />
      )}
      {restoring && (
        <Confirm
          title="Restore this backup?"
          lead={`${when(restoring)} · ${backupLabel(restoring)}`}
          body={<p>The current data is backed up first, then replaced by this copy. Recording restarts on it.</p>}
          confirm="Restore"
          onConfirm={() => restore(restoring)}
          onClose={() => setRestoring(null)}
        />
      )}
    </section>
  );
}
