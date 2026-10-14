const pad = (n: number) => String(n).padStart(2, "0");

/** `8:32` under an hour, `1:05:09` above. */
export function elapsed(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  return h > 0 ? `${h}:${pad(m)}:${pad(s % 60)}` : `${m}:${pad(s % 60)}`;
}

/** `1:05` (hours:minutes), for the session pill. */
export function hoursMinutes(ms: number): string {
  const m = Math.max(0, Math.floor(ms / 60_000));
  return `${Math.floor(m / 60)}:${pad(m % 60)}`;
}

/** `2h 05`, or `45 min` under an hour. */
export function longDuration(ms: number): string {
  const m = Math.max(0, Math.round(ms / 60_000));
  return m < 60 ? `${m} min` : `${Math.floor(m / 60)}h ${pad(m % 60)}`;
}

/** Match clock: `2:47`. */
export function clock(seconds: number): string {
  const s = Math.max(0, Math.round(seconds));
  return `${Math.floor(s / 60)}:${pad(s % 60)}`;
}

export const whole = (n: number) => String(Math.round(n));
export const kmh = (n: number) => String(Math.round(n));
/** A 0–1 rate as a whole percentage, or `—`. */
export const ratePercent = (rate: number | null) => (rate == null ? "—" : String(Math.round(rate * 100)));

const ARENAS: Record<string, string> = {
  stadium_p: "DFH Stadium",
  stadium_day_p: "DFH Stadium (Day)",
  stadium_foggy_p: "DFH Stadium (Stormy)",
  stadium_winter_p: "DFH Stadium (Snowy)",
  eurostadium_p: "Mannfield",
  eurostadium_night_p: "Mannfield (Night)",
  eurostadium_rainy_p: "Mannfield (Stormy)",
  eurostadium_snownight_p: "Mannfield (Snowy)",
  park_p: "Beckwith Park",
  park_night_p: "Beckwith Park (Midnight)",
  park_rainy_p: "Beckwith Park (Stormy)",
  trainstation_p: "Urban Central",
  trainstation_night_p: "Urban Central (Night)",
  trainstation_dawn_p: "Urban Central (Dawn)",
  utopiastadium_p: "Utopia Coliseum",
  utopiastadium_dusk_p: "Utopia Coliseum (Dusk)",
  utopiastadium_snow_p: "Utopia Coliseum (Snowy)",
  cs_p: "Champions Field",
  cs_day_p: "Champions Field (Day)",
  cs_hw_p: "Rivals Arena",
  wasteland_s_p: "Wasteland",
  wasteland_night_s_p: "Wasteland (Night)",
  neotokyo_standard_p: "Neo Tokyo",
  arc_standard_p: "Starbase ARC",
  farm_p: "Farmstead",
  beach_p: "Salty Shores",
  beach_night_p: "Salty Shores (Night)",
  underwater_p: "AquaDome",
  chn_stadium_p: "Forbidden Temple",
  street_p: "Sovereign Heights",
  throwbackstadium_p: "Throwback Stadium",
  ff_dusk_p: "Estadio Vida",
};

/** Display name of an arena code like `Stadium_P`; unknown codes are tidied up. */
export function arenaName(code: string): string {
  const known = ARENAS[code.toLowerCase()];
  if (known) return known;
  return code
    .replace(/_p$/i, "")
    .split("_")
    .filter(Boolean)
    .map((w) => w[0].toUpperCase() + w.slice(1))
    .join(" ");
}

/** The playlist id of private matches (the game reports them as unranked). */
export const PRIVATE_PLAYLIST = 6;

export function modeLabel(m: { ranked: boolean; team_size: number | null; playlist_id?: number | null }): string {
  if (m.team_size == null) return "Match";
  const kind = m.ranked ? "Ranked" : m.playlist_id === PRIVATE_PLAYLIST ? "Private" : "Casual";
  return `${kind} ${m.team_size}v${m.team_size}`;
}

const startOfDay = (ms: number) => {
  const d = new Date(ms);
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
};

export function dayLabel(ms: number, now: number): string {
  const days = Math.round((startOfDay(now) - startOfDay(ms)) / 86_400_000);
  if (days === 0) return "Today";
  if (days === 1) return "Yesterday";
  return new Date(ms).toLocaleDateString("en-GB", { weekday: "short", day: "numeric", month: "short" });
}

export function timeOfDay(ms: number): string {
  const d = new Date(ms);
  return `${pad(d.getHours())}:${pad(d.getMinutes())}`;
}
