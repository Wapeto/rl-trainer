import { QrCode } from "lucide-react";

export function Pairing() {
  return (
    <div className="screen screen--center">
      <QrCode size={48} className="pairing__icon" aria-hidden />
      <h1 className="title">Scan the QR code on the PC</h1>
      <p className="pairing__text">
        On the PC, open RL Trainer and choose <b>Enable phone access</b>. Scan the code with this phone while it is on
        the same Wi-Fi.
      </p>
    </div>
  );
}
