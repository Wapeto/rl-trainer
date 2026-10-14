import { useState } from "react";
import type { Notify } from "@shared/components/Toast";
import type { PackInput, PackRow } from "@shared/generated/types";
import { codeProblem, formatCode } from "@shared/packCode";
import { sentence } from "../act";
import { errorText, type DesktopApi } from "../api";
import { Dialog } from "../components/Dialog";
import { useLoad } from "../components/useLoad";
import { Row, Switch } from "../settings/fields";
import { CodeField } from "./CodeField";

export const NAME_MAX = 80;
export const NOTES_MAX = 2000;

export interface PackFields {
  code: string;
  name: string;
  /** Whole percent, as typed. */
  target: string;
  shots: string;
  /** Comma separated. */
  tags: string;
  notes: string;
  /** A shot is made unless the ball goes in. */
  goalie: boolean;
}

function initial(pack: PackRow | null): PackFields {
  if (!pack) return { code: "", name: "", target: "65", shots: "", tags: "", notes: "", goalie: false };
  return {
    code: pack.code,
    name: pack.name,
    target: String(Math.round(pack.target_rate * 100)),
    shots: pack.shot_count?.toString() ?? "",
    tags: pack.tags.join(", "),
    notes: pack.notes,
    goalie: pack.goalie,
  };
}

const wholeIn = (raw: string, min: number, max: number) => /^\d+$/.test(raw) && Number(raw) >= min && Number(raw) <= max;

/** The pack to send, or the first problem with the fields (the app checks again). */
export function toInput(f: PackFields): PackInput | string {
  const codeError = codeProblem(f.code);
  if (codeError) return codeError;
  const name = f.name.trim();
  const chars = [...name].length;
  if (chars === 0 || chars > NAME_MAX) return `The name needs 1 to ${NAME_MAX} characters`;
  const target = f.target.trim();
  if (!wholeIn(target, 1, 100)) return "The target is a whole percentage from 1 to 100";
  const shots = f.shots.trim();
  if (shots !== "" && !wholeIn(shots, 1, 100)) return "Shots are a whole number from 1 to 100";
  return {
    code: formatCode(f.code),
    name,
    target_rate: Number(target) / 100,
    shot_count: shots === "" ? null : Number(shots),
    tags: f.tags
      .split(",")
      .map((t) => t.trim())
      .filter(Boolean),
    notes: f.notes,
    goalie: f.goalie,
  };
}

interface PackDialogProps {
  api: DesktopApi;
  notify: Notify;
  /** `null` adds a pack; otherwise this one is edited. */
  pack: PackRow | null;
  onSaved: (id: number) => void;
  /** Open another pack (the one that already has the typed code). */
  onOpen: (id: number) => void;
  onClose: () => void;
}

export function PackDialog({ api, notify, pack, onSaved, onOpen, onClose }: PackDialogProps) {
  const [fields, setFields] = useState<PackFields>(() => initial(pack));
  const [codeLeft, setCodeLeft] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const all = useLoad(() => api.packs("name", ""), "all").data ?? [];
  const set = (key: Exclude<keyof PackFields, "goalie">) => (value: string) => {
    setFields({ ...fields, [key]: value });
    setError(null);
  };
  const duplicate =
    codeProblem(fields.code) == null
      ? (all.find((p) => p.pack.id !== pack?.id && p.pack.code === formatCode(fields.code))?.pack ?? null)
      : null;

  async function save() {
    setCodeLeft(true);
    const input = toInput(fields);
    if (typeof input === "string") {
      setError(input);
      return;
    }
    if (duplicate) return;
    setBusy(true);
    try {
      let id: number;
      if (pack) {
        await api.updatePack(pack.id, input);
        id = pack.id;
      } else {
        id = await api.addPack(input);
      }
      notify(pack ? "Pack saved" : "Pack added", "ok");
      onSaved(id);
      onClose();
    } catch (err) {
      setError(sentence(errorText(err)));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog title={pack ? "Edit pack" : "Add a pack"} onClose={onClose}>
      <CodeField
        value={fields.code}
        onChange={set("code")}
        onBlur={() => setCodeLeft(true)}
        problem={codeLeft ? codeProblem(fields.code) : null}
        duplicateOf={duplicate}
        onOpen={(id) => {
          onClose();
          onOpen(id);
        }}
      />
      <label className="d-field">
        <span className="d-k">Name</span>
        <input className="d-input" aria-label="Name" value={fields.name} onChange={(e) => set("name")(e.target.value)} />
      </label>
      <div className="d-row2">
        <label className="d-field">
          <span className="d-k">Target make rate (%)</span>
          <input
            className="d-input"
            aria-label="Target make rate"
            inputMode="numeric"
            value={fields.target}
            onChange={(e) => set("target")(e.target.value)}
          />
        </label>
        <label className="d-field">
          <span className="d-k">Shots</span>
          <input
            className="d-input"
            aria-label="Shots"
            inputMode="numeric"
            placeholder="Optional"
            value={fields.shots}
            onChange={(e) => set("shots")(e.target.value)}
          />
        </label>
      </div>
      <label className="d-field">
        <span className="d-k">Tags</span>
        <input
          className="d-input"
          aria-label="Tags"
          placeholder="aerial, passing"
          value={fields.tags}
          onChange={(e) => set("tags")(e.target.value)}
        />
      </label>
      <Row title="Goalie pack" hint="A shot counts as made unless the ball goes in.">
        <Switch label="Goalie pack" on={fields.goalie} onChange={(goalie) => setFields({ ...fields, goalie })} />
      </Row>
      <label className="d-field">
        <span className="d-k">Note</span>
        <textarea
          className="d-input"
          aria-label="Note"
          placeholder="Optional"
          maxLength={NOTES_MAX}
          value={fields.notes}
          onChange={(e) => set("notes")(e.target.value)}
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
          {pack ? "Save" : "Add pack"}
        </button>
      </div>
    </Dialog>
  );
}
