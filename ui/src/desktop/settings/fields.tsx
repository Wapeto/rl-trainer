import { useEffect, useState, type ReactNode } from "react";

export function Switch({ label, on, disabled, onChange }: { label: string; on: boolean; disabled?: boolean; onChange: (on: boolean) => void }) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      className="d-switch"
      disabled={disabled}
      onClick={() => onChange(!on)}
    />
  );
}

export function Row({ title, hint, children }: { title: string; hint?: ReactNode; children?: ReactNode }) {
  return (
    <div className="d-srow">
      <div className="d-srow__tx">
        <b>{title}</b>
        {hint && <span>{hint}</span>}
      </div>
      {children}
    </div>
  );
}

interface EditSettingProps {
  label: string;
  value: string;
  unit?: string;
  wide?: boolean;
  numeric?: boolean;
  placeholder?: string;
  /** Saves the trimmed text; returns the error to show, or null. */
  onSave: (raw: string) => Promise<string | null>;
}

/** A value with its own Save button, shown once the text differs from the saved value. */
export function EditSetting({ label, value, unit, wide, numeric, placeholder, onSave }: EditSettingProps) {
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  useEffect(() => setDraft(value), [value]);
  const dirty = draft.trim() !== value;

  async function save() {
    setBusy(true);
    const problem = await onSave(draft.trim());
    setBusy(false);
    setError(problem);
  }

  return (
    <div className="d-edit">
      <input
        className={`d-input ${wide ? "d-path" : "d-num"}${error ? " d-input--bad" : ""}`}
        aria-label={label}
        inputMode={numeric ? "numeric" : undefined}
        placeholder={placeholder}
        value={draft}
        onChange={(e) => {
          setDraft(e.target.value);
          setError(null);
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && dirty) void save();
        }}
      />
      {unit && <span className="d-m">{unit}</span>}
      {dirty && (
        <button type="button" className="d-btn d-btn--primary" disabled={busy} onClick={save}>
          Save
        </button>
      )}
      {error && (
        <p className="d-err" role="alert">
          {error}
        </p>
      )}
    </div>
  );
}
