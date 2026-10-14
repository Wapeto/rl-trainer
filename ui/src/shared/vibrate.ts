/** A short buzz on phones that allow it (Android Chrome after a tap on the page); nothing elsewhere. */
export function vibrate(pattern: number): void {
  try {
    navigator.vibrate?.(pattern);
  } catch {
    // Some browsers throw when vibration is blocked; a missing buzz is fine.
  }
}
