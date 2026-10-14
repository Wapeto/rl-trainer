import { describe, expect, it, vi } from "vitest";
import { TOKEN_KEY, takeToken } from "./token";

function storage(initial: Record<string, string> = {}) {
  const data = new Map(Object.entries(initial));
  return {
    getItem: (k: string) => data.get(k) ?? null,
    setItem: (k: string, v: string) => void data.set(k, v),
    data,
  };
}

describe("takeToken", () => {
  it("stores the token from the fragment and removes it from the address bar", () => {
    const store = storage();
    const history = { replaceState: vi.fn() };
    const loc = { hash: "#t=abc123", pathname: "/", search: "" };
    expect(takeToken(loc as Location, store as unknown as Storage, history as unknown as History)).toBe("abc123");
    expect(store.data.get(TOKEN_KEY)).toBe("abc123");
    expect(history.replaceState).toHaveBeenCalledWith(null, "", "/");
  });

  it("falls back to the stored token, or null when unpaired", () => {
    const loc = { hash: "", pathname: "/", search: "" } as Location;
    const history = { replaceState: vi.fn() } as unknown as History;
    expect(takeToken(loc, storage({ [TOKEN_KEY]: "saved" }) as unknown as Storage, history)).toBe("saved");
    expect(takeToken(loc, storage() as unknown as Storage, history)).toBeNull();
  });
});
