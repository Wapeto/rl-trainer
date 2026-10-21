import { useState } from "react";
import type { Notify } from "@shared/components/Toast";
import { act } from "./act";
import type { DesktopApi } from "./api";
import { Dialog } from "./components/Dialog";
import { useLoad } from "./components/useLoad";
import { Pairing } from "./settings/Pairing";

interface PairingDialogProps {
  api: DesktopApi;
  notify: Notify;
  phones: number;
  onClose: () => void;
}

/** The sidebar's phone line opens this: the pairing code, or a way to turn the phone page on. */
export function PairingDialog({ api, notify, phones, onClose }: PairingDialogProps) {
  const [tick, setTick] = useState(0);
  const refresh = () => setTick((n) => n + 1);
  const settings = useLoad(() => api.settings(), String(tick));
  const pairing = useLoad(() => api.pairing(), String(tick));
  async function turnOn() {
    if (await act(notify, () => api.enablePhone(), "Phone page on")) refresh();
  }
  const on = settings.data?.phone_enabled && pairing.data;
  return (
    <Dialog title="Pair a phone" onClose={onClose}>
      {on ? (
        <Pairing api={api} pairing={pairing.data!} phones={phones} notify={notify} refresh={refresh} />
      ) : (
        <div className="d-confirm">
          <p>
            {settings.data
              ? settings.data.phone_enabled
                ? (pairing.error ?? "Loading…")
                : "The phone page is off."
              : (settings.error ?? "Loading…")}
          </p>
          {settings.data && !settings.data.phone_enabled && (
            <button type="button" className="d-btn d-btn--primary" onClick={turnOn}>
              Turn on phone page
            </button>
          )}
        </div>
      )}
      <div className="d-actions">
        <button type="button" className="d-btn" onClick={onClose}>
          Close
        </button>
      </div>
    </Dialog>
  );
}
