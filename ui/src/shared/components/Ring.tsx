import { useId, type ReactNode } from "react";

export type Tone = "amber" | "green" | "blue" | "gold" | "red" | "neutral";

const GRADIENTS: Record<Tone, [string, string]> = {
  amber: ["#ffd59a", "#f0a95a"],
  green: ["#a6f5cf", "#3fcf8e"],
  blue: ["#c3d6ff", "#6f97ff"],
  gold: ["#ffe2a6", "#ffcf7a"],
  red: ["#ff9aa6", "#f0727f"],
  neutral: ["#c9d3ff", "#9ab4ff"],
};

const SIZES = {
  hero: { box: 120, r: 53, stroke: 4.5, px: 224 },
  tile: { box: 96, r: 42, stroke: 6, px: 96 },
  mini: { box: 44, r: 18, stroke: 4, px: 30 },
};

interface RingProps {
  /** 0–1; clamped. */
  value: number;
  /** 0–1; drawn as a dot on the ring. */
  target?: number | null;
  tone: Tone;
  size: "hero" | "tile" | "mini";
  label: string;
  children?: ReactNode;
}

export function Ring({ value, target, tone, size, label, children }: RingProps) {
  const gradientId = `ring${useId().replace(/[^a-zA-Z0-9]/g, "")}`;
  const { box, r, stroke, px } = SIZES[size];
  const mid = box / 2;
  const circumference = 2 * Math.PI * r;
  const v = Math.min(1, Math.max(0, value));
  return (
    <div className={`ring ring--${size}`} style={{ width: px, height: px }} role="img" aria-label={label}>
      <svg viewBox={`0 0 ${box} ${box}`} width={px} height={px} aria-hidden>
        <defs>
          <linearGradient id={gradientId} x1="0" y1="0" x2="1" y2="1">
            <stop offset="0" stopColor={GRADIENTS[tone][0]} />
            <stop offset="1" stopColor={GRADIENTS[tone][1]} />
          </linearGradient>
        </defs>
        <circle cx={mid} cy={mid} r={r} fill="none" className="ring__track" strokeWidth={stroke} />
        {v > 0 && (
          <circle
            data-testid="ring-arc"
            cx={mid}
            cy={mid}
            r={r}
            fill="none"
            stroke={`url(#${gradientId})`}
            strokeWidth={stroke}
            strokeLinecap="round"
            strokeDasharray={`${(circumference * v).toFixed(2)} ${circumference.toFixed(2)}`}
            transform={`rotate(-90 ${mid} ${mid})`}
          />
        )}
        {target != null && (
          <circle
            data-testid="ring-target"
            cx={mid}
            cy={mid - r}
            r={size === "hero" ? 3.2 : 3}
            className="ring__target"
            transform={`rotate(${Math.round(target * 360)} ${mid} ${mid})`}
          />
        )}
      </svg>
      <div className="ring__center">{children}</div>
    </div>
  );
}
