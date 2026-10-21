import { Copy } from "lucide-react";
import type { Notify } from "@shared/components/Toast";
import type { PairingInfo } from "@shared/generated/types";
import { act } from "../act";
import type { DesktopApi } from "../api";
import { plural } from "../format";

export const maskToken = (url: string) => url.replace(/#t=.*/, "#t=••••••••");

interface PairingProps {
  api: DesktopApi;
  pairing: PairingInfo;
  phones: number;
  notify: Notify;
  /** Called after the pairing changed (a new code), so the page can reload it. */
  refresh: () => void;
}

/** The QR code, the masked link, copy and a new code: shown in Settings and in the pairing dialog. */
export function Pairing({ api, pairing, phones, notify, refresh }: PairingProps) {
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
  );
}
