import { useEffect, useId, useState, type ReactNode } from "react";

interface DialogProps {
  title: string;
  lead?: ReactNode;
  onClose: () => void;
  children: ReactNode;
}

/** A centred window over a dimmed page; Escape or a click outside closes it. */
export function Dialog({ title, lead, onClose, children }: DialogProps) {
  const id = useId();
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="d-scrim" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="d-dialog" role="dialog" aria-modal="true" aria-labelledby={id}>
        <h2 id={id}>{title}</h2>
        {lead && <p className="d-lead">{lead}</p>}
        {children}
      </div>
    </div>
  );
}

interface ConfirmProps {
  title: string;
  lead?: ReactNode;
  body: ReactNode;
  confirm: string;
  /** Shown on the confirm button while `onConfirm` runs. */
  busyLabel?: string;
  danger?: boolean;
  onConfirm: () => Promise<unknown>;
  onClose: () => void;
}

export function Confirm({ title, lead, body, confirm, busyLabel, danger, onConfirm, onClose }: ConfirmProps) {
  const [busy, setBusy] = useState(false);
  async function run() {
    if (busy) return;
    setBusy(true);
    try {
      await onConfirm();
    } finally {
      setBusy(false);
    }
  }
  return (
    <Dialog title={title} lead={lead} onClose={onClose}>
      <div className="d-confirm">{body}</div>
      <div className="d-actions">
        <button type="button" className="d-btn" onClick={onClose}>
          Cancel
        </button>
        <button
          type="button"
          className={`d-btn ${danger ? "d-btn--danger" : "d-btn--primary"}`}
          disabled={busy}
          onClick={run}
        >
          {busy && busyLabel ? busyLabel : confirm}
        </button>
      </div>
    </Dialog>
  );
}
