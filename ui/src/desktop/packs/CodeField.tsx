import { useId } from "react";
import type { PackRow } from "@shared/generated/types";
import { formatCode } from "@shared/packCode";

interface CodeFieldProps {
  value: string;
  onChange: (value: string) => void;
  onBlur: () => void;
  /** Why the code is not valid (shown once the field was left), or null. */
  problem: string | null;
  /** A saved pack that already has this code. */
  duplicateOf: PackRow | null;
  onOpen: (id: number) => void;
}

/** A pack code typed any way: upper-cased, with a dash every four characters. */
export function CodeField({ value, onChange, onBlur, problem, duplicateOf, onOpen }: CodeFieldProps) {
  const hint = useId();
  const bad = problem != null || duplicateOf != null;
  return (
    <div className="d-field">
      <label>
        <span className="d-k">Code</span>
        <input
          className={`d-input d-input--code${bad ? " d-input--bad" : ""}`}
          aria-label="Code"
          placeholder="C7E0-9E0B-B739-A899"
          value={value}
          spellCheck={false}
          autoComplete="off"
          aria-invalid={bad}
          aria-describedby={bad ? hint : undefined}
          onChange={(e) => onChange(formatCode(e.target.value))}
          onBlur={onBlur}
        />
      </label>
      {problem && (
        <p className="d-err" id={hint}>
          {problem}
        </p>
      )}
      {!problem && duplicateOf && (
        <p className="d-err" id={hint}>
          Already saved as {duplicateOf.name}.{" "}
          <button type="button" className="d-link" onClick={() => onOpen(duplicateOf.id)}>
            Open it
          </button>
        </p>
      )}
    </div>
  );
}
