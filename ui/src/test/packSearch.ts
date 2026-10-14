import type { PackStats } from "@shared/generated/types";

/** What the app's Rust search returns, for fakes: name or tag contains the query, or the code
 * does once case and dashes are ignored. */
export function searchPacks(packs: PackStats[], search = ""): PackStats[] {
  const q = search.trim().toLowerCase();
  if (!q) return packs;
  const key = (s: string) => s.replace(/[^0-9a-z]/gi, "").toUpperCase();
  return packs.filter(
    (p) =>
      p.pack.name.toLowerCase().includes(q) ||
      p.pack.tags.some((t) => t.toLowerCase().includes(q)) ||
      (key(search) !== "" && key(p.pack.code).includes(key(search))),
  );
}
