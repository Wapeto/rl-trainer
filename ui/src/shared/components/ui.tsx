import type { ButtonHTMLAttributes, HTMLAttributes, ReactNode } from "react";

export function Glass({ className = "", ...rest }: HTMLAttributes<HTMLDivElement>) {
  return <div className={`glass ${className}`.trim()} {...rest} />;
}

export function Pill({ live, off, children }: { live?: boolean; off?: boolean; children: ReactNode }) {
  return <span className={`pill glass${live ? " pill--live" : ""}${off ? " pill--off" : ""}`}>{children}</span>;
}

export function StatRow({ items }: { items: { label: string; value: ReactNode }[] }) {
  return (
    <div className={`stat-row glass stat-row--${items.length}`}>
      {items.map((item) => (
        <div key={item.label} className="stat">
          <span className="stat__label">{item.label}</span>
          <b className="stat__value">{item.value}</b>
        </div>
      ))}
    </div>
  );
}

export function Kv({ label, value }: { label: string; value: ReactNode }) {
  return (
    <div className="kv">
      <span>{label}</span>
      <b>{value}</b>
    </div>
  );
}

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "text" };

export function Button({ variant = "secondary", className = "", ...rest }: ButtonProps) {
  return <button type="button" className={`btn btn--${variant} ${className}`.trim()} {...rest} />;
}
