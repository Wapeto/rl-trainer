const stripped = (raw: string) => raw.replace(/[\s-]/g, "").toUpperCase();

/** What the code field shows while typing: upper case with a dash every four characters. Extra
 * or wrong characters are kept so the field can say what is wrong. */
export function formatCode(raw: string): string {
  return (stripped(raw).match(/.{1,4}/g) ?? []).join("-");
}

/** Why a code is not valid yet, or null. Mirrors `normalize_pack_code` in Rust. */
export function codeProblem(raw: string): string | null {
  const key = stripped(raw);
  if (!/^[0-9A-F]*$/.test(key)) return "Only 0–9 and A–F";
  if (key.length !== 16) return `${key.length} of 16 characters`;
  return null;
}
