import { useState } from "react";
import type { Notify } from "@shared/components/Toast";
import type { MmrInput, SessionDetail } from "@shared/generated/types";
import { act } from "../act";
import type { DesktopApi } from "../api";
import { Dialog } from "../components/Dialog";
import { longDay } from "../format";

export const MMR_MAX = 3000;
export const NOTES_MAX = 2000;

/** Ranked team sizes played in the session, plus sizes that already hold an entry in it. */
export function reviewSizes(detail: SessionDetail): number[] {
  const sizes = new Set<number>();
  for (const a of detail.activities) {
    const m = a.match_summary;
    if (m?.ranked && m.team_size != null) sizes.add(m.team_size);
  }
  for (const e of detail.mmr) sizes.add(e.team_size);
  return [...sizes].sort((a, b) => a - b);
}

/** The session's last MMR entry for a team size. */
export function lastMmr(detail: SessionDetail, size: number): number | null {
  const rows = detail.mmr.filter((e) => e.team_size === size);
  return rows.length > 0 ? rows[rows.length - 1].mmr : null;
}

interface EditReviewDialogProps {
  api: DesktopApi;
  detail: SessionDetail;
  notify: Notify;
  onDone: () => void;
  onClose: () => void;
}

export function EditReviewDialog({ api, detail, notify, onDone, onClose }: EditReviewDialogProps) {
  const sizes = reviewSizes(detail);
  const [values, setValues] = useState<Record<number, string>>(() =>
    Object.fromEntries(sizes.map((size) => [size, lastMmr(detail, size)?.toString() ?? ""])),
  );
  const [notes, setNotes] = useState(detail.notes);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  async function save() {
    const mmr: MmrInput[] = [];
    for (const size of sizes) {
      const raw = (values[size] ?? "").trim();
      if (raw === "") continue;
      const n = Number(raw);
      if (!/^\d+$/.test(raw) || n > MMR_MAX) {
        setError(`${size}v${size} MMR must be a whole number from 0 to ${MMR_MAX}`);
        return;
      }
      mmr.push({ team_size: size, mmr: n });
    }
    setError(null);
    setBusy(true);
    const ok = await act(notify, () => api.editReview(detail.id, mmr, notes), "Review saved");
    setBusy(false);
    if (ok) {
      onDone();
      onClose();
    }
  }

  return (
    <Dialog
      title={`Review · ${longDay(detail.started_at)}`}
      lead={sizes.length > 0 ? "One MMR field per ranked mode you played in this session." : "No ranked match in this session."}
      onClose={onClose}
    >
      {sizes.length > 0 && (
        <div className="d-row2">
          {sizes.map((size) => (
            <label key={size} className="d-field">
              <span className="d-k">{`${size}v${size} MMR`}</span>
              <input
                className="d-input"
                inputMode="numeric"
                aria-label={`${size}v${size} MMR`}
                value={values[size] ?? ""}
                onChange={(e) => setValues({ ...values, [size]: e.target.value })}
              />
            </label>
          ))}
        </div>
      )}
      <label className="d-field">
        <span className="d-k">Note</span>
        <textarea
          className="d-input"
          aria-label="Note"
          maxLength={NOTES_MAX}
          value={notes}
          onChange={(e) => setNotes(e.target.value)}
        />
      </label>
      {error && (
        <p className="d-err" role="alert">
          {error}
        </p>
      )}
      <div className="d-actions">
        <button type="button" className="d-btn" onClick={onClose}>
          Cancel
        </button>
        <button type="button" className="d-btn d-btn--primary" disabled={busy} onClick={save}>
          Save
        </button>
      </div>
    </Dialog>
  );
}
