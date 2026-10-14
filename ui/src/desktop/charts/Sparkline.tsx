interface SparklineProps {
  values: number[];
  color: string;
  width?: number;
  height?: number;
}

/** A tiny trend line for list lanes and mode cards; needs two values. */
export function Sparkline({ values, color, width = 44, height = 18 }: SparklineProps) {
  if (values.length < 2) return <svg className="c-spark" width={width} height={height} aria-hidden />;
  const lo = Math.min(...values);
  const span = Math.max(...values) - lo || 1;
  const points = values
    .map((v, i) => `${(i * width) / (values.length - 1)},${height - 2 - ((height - 4) * (v - lo)) / span}`)
    .join(" ");
  return (
    <svg className="c-spark" width={width} height={height} aria-hidden>
      <polyline fill="none" stroke={color} strokeWidth={1.4} points={points} />
    </svg>
  );
}
