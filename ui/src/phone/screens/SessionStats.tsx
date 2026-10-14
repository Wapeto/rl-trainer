import { StatRow } from "@shared/components/ui";
import { ratePercent } from "@shared/format";
import type { SessionSummary } from "@shared/generated/types";

/** Matches, training make rate and activities of a finished session. */
export function SessionStats({ s }: { s: SessionSummary }) {
  const played = s.wins + s.losses > 0;
  return (
    <StatRow
      items={[
        { label: "Matches", value: played ? `W${s.wins} · L${s.losses}` : "—" },
        { label: "Training made", value: s.training_make_rate == null ? "—" : `${ratePercent(s.training_make_rate)}%` },
        { label: "Activities", value: s.activities },
      ]}
    />
  );
}
