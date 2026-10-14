/** The game keeps the newest 20 online matches in Match History, for up to 30 days. */
export const HISTORY_SIZE = 20;
export const HISTORY_DAYS = 30;
/** From this place on a match is about to drop out of history. */
export const SOON_GONE_PLACE = 17;
const DAY_MS = 86_400_000;

export function inHistory(place: number, startedAt: number, now: number): boolean {
  return place <= HISTORY_SIZE && now - startedAt < HISTORY_DAYS * DAY_MS;
}
