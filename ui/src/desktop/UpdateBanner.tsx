import { useState } from "react";
import type { Notify } from "@shared/components/Toast";
import { act } from "./act";
import type { DesktopApi, UpdateInfo } from "./api";
import { Confirm } from "./components/Dialog";

interface Props {
  api: DesktopApi;
  update: UpdateInfo;
  gameRunning: boolean;
  notify: Notify;
}

/** Never installs by itself: the user presses the button, and is told when a game is running. */
export function UpdateBanner({ api, update, gameRunning, notify }: Props) {
  const [asking, setAsking] = useState(false);
  const install = () => act(notify, () => api.installUpdate());
  return (
    <div className="d-update glass" role="status">
      <span>{`Version ${update.version} is ready`}</span>
      <button type="button" className="d-btn d-btn--primary" onClick={() => (gameRunning ? setAsking(true) : void install())}>
        Install and restart
      </button>
      {asking && (
        <Confirm
          title="Install and restart?"
          body={<p>Rocket League is running. RL Trainer restarts to install the update, and the current session ends.</p>}
          confirm="Install and restart"
          busyLabel="Installing…"
          onConfirm={install}
          onClose={() => setAsking(false)}
        />
      )}
    </div>
  );
}
