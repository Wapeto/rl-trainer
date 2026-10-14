import { Ellipsis } from "lucide-react";
import { Fragment, useEffect, useRef, useState, type ReactNode } from "react";

export interface MenuItem {
  label: string;
  icon?: ReactNode;
  onSelect: () => void;
  disabled?: boolean;
  danger?: boolean;
  separatorBefore?: boolean;
}

/** A "⋯" button with a small menu; a click elsewhere closes it. */
export function MenuButton({ label, items }: { label: string; items: MenuItem[] }) {
  const [open, setOpen] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", onDown);
    return () => document.removeEventListener("mousedown", onDown);
  }, [open]);
  return (
    <div className="d-menuwrap" ref={ref}>
      <button
        type="button"
        className="d-btn d-btn--icon"
        aria-label={label}
        aria-haspopup="menu"
        aria-expanded={open}
        onClick={() => setOpen(!open)}
      >
        <Ellipsis className="d-icon" aria-hidden />
      </button>
      {open && (
        <div className="d-menu" role="menu">
          {items.map((item) => (
            <Fragment key={item.label}>
              {item.separatorBefore && <div className="d-sep" role="separator" />}
              <button
                type="button"
                role="menuitem"
                className={`d-mi${item.danger ? " d-mi--danger" : ""}`}
                disabled={item.disabled}
                onClick={() => {
                  setOpen(false);
                  item.onSelect();
                }}
              >
                {item.icon}
                {item.label}
              </button>
            </Fragment>
          ))}
        </div>
      )}
    </div>
  );
}
