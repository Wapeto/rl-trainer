import { ChevronDown } from "lucide-react";
import { useEffect, useRef, useState } from "react";

export interface SelectOption<T extends string> {
  value: T;
  label: string;
}

interface SelectProps<T extends string> {
  /** The accessible name of the button and of its list. */
  label: string;
  value: T;
  options: SelectOption<T>[];
  onChange: (value: T) => void;
  /** `inline`: plain text in a group header; `field` (default): a bordered field. */
  variant?: "field" | "inline";
}

/** A choice drawn with the window's own menu, not the system drop-down. */
export function Select<T extends string>({ label, value, options, onChange, variant = "field" }: SelectProps<T>) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    window.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      window.removeEventListener("keydown", onKey);
    };
  }, [open]);
  const current = options.find((o) => o.value === value)?.label ?? value;
  return (
    <div className="d-menuwrap" ref={ref}>
      <button
        type="button"
        className={`d-pickbtn d-pickbtn--${variant}`}
        aria-label={label}
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <span>{current}</span>
        <ChevronDown className="d-pickbtn__chev" aria-hidden />
      </button>
      {open && (
        <div className="d-menu d-menu--pick" role="listbox" aria-label={label}>
          {options.map((o) => (
            <button
              key={o.value}
              type="button"
              role="option"
              aria-selected={o.value === value}
              className={`d-mi${o.value === value ? " d-mi--on" : ""}`}
              onClick={() => {
                setOpen(false);
                if (o.value !== value) onChange(o.value);
              }}
            >
              {o.label}
            </button>
          ))}
        </div>
      )}
    </div>
  );
}
