export const TOKEN_KEY = "rl-trainer-token";

/** Reads the pairing token from `#t=` (then drops it from the address bar) or from storage. */
export function takeToken(
  loc: Pick<Location, "hash" | "pathname" | "search"> = window.location,
  storage: Pick<Storage, "getItem" | "setItem"> = window.localStorage,
  history: Pick<History, "replaceState"> = window.history,
): string | null {
  const fromUrl = new URLSearchParams(loc.hash.slice(1)).get("t");
  if (fromUrl) {
    storage.setItem(TOKEN_KEY, fromUrl);
    history.replaceState(null, "", loc.pathname + loc.search);
    return fromUrl;
  }
  return storage.getItem(TOKEN_KEY);
}
