/** What the window remembers between runs (local storage, or nothing when it is blocked). */
export type KeyValueStore = Pick<Storage, "getItem" | "setItem">;

export function browserStorage(): KeyValueStore | null {
  try {
    return window.localStorage;
  } catch {
    return null;
  }
}

export function readKey(storage: KeyValueStore | null, key: string): string | null {
  try {
    return storage?.getItem(key) ?? null;
  } catch {
    return null;
  }
}

export function writeKey(storage: KeyValueStore | null, key: string, value: string): void {
  try {
    storage?.setItem(key, value);
  } catch {
    // Not remembered (storage blocked); the choice still applies now.
  }
}
