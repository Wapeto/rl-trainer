// Receives reports from RL Trainer and forwards them to a Discord channel.
// Limits mirror crates/rl-app/src/report.rs.

const KINDS = ["bug", "idea"];
const MAX_TEXT = 2000;
const MAX_CONTACT = 64;
const MAX_SHOT = 8 * 1024 * 1024;
const MAX_LOG = 300 * 1024;
const MAX_TOTAL = 9 * 1024 * 1024;
const MAX_FIELD = 64; // version / windows strings

const PNG = [0x89, 0x50, 0x4e, 0x47];
const JPEG = [0xff, 0xd8, 0xff];
const startsWith = (bytes, magic) => magic.every((b, i) => bytes[i] === b);

/** Checks a report; returns { ok: true, report } or { ok: false, error }. */
export async function validateReport(form, headers) {
  const bad = (error) => ({ ok: false, error });
  const version = headers.get("X-RL-Trainer-Version");
  if (!version || version.length > MAX_FIELD) return bad("missing version header");
  const kind = form.get("kind");
  if (!KINDS.includes(kind)) return bad("kind must be bug or idea");
  const text = String(form.get("text") ?? "").trim();
  if (text.length < 1 || [...text].length > MAX_TEXT) return bad("text must be 1 to 2000 characters");
  const contact = String(form.get("contact") ?? "").trim();
  if ([...contact].length > MAX_CONTACT) return bad("contact is too long");
  const windows = String(form.get("windows") ?? "").slice(0, MAX_FIELD);

  let total = 0;
  let screenshot = null;
  const shot = form.get("screenshot");
  if (shot instanceof Blob) {
    if (shot.size > MAX_SHOT) return bad("screenshot is bigger than 8 MB");
    const bytes = new Uint8Array(await shot.slice(0, 4).arrayBuffer());
    const type = startsWith(bytes, PNG) ? "image/png" : startsWith(bytes, JPEG) ? "image/jpeg" : null;
    if (!type) return bad("screenshot must be a PNG or JPEG image");
    total += shot.size;
    screenshot = { blob: shot, name: type === "image/png" ? "screenshot.png" : "screenshot.jpg" };
  }
  let log = null;
  const logFile = form.get("log");
  if (logFile instanceof Blob) {
    if (logFile.size > MAX_LOG) return bad("log is bigger than 300 KB");
    total += logFile.size;
    log = { blob: logFile, name: "log.txt" };
  }
  if (total > MAX_TOTAL) return bad("report is too big");
  return { ok: true, report: { kind, text, contact, version, windows, screenshot, log } };
}

/** The Discord webhook body: an embed, plus the files. */
export function discordForm(report) {
  const embed = {
    title: report.kind === "bug" ? "Bug report" : "Idea",
    description: report.text,
    color: report.kind === "bug" ? 0xe5484d : 0x30a46c,
    fields: [
      { name: "Version", value: report.version, inline: true },
      { name: "Windows", value: report.windows || "unknown", inline: true },
      { name: "From", value: report.contact || "anonymous", inline: true },
    ],
  };
  const out = new FormData();
  out.set("payload_json", JSON.stringify({ embeds: [embed] }));
  [report.screenshot, report.log].filter(Boolean).forEach((file, n) => {
    out.set(`files[${n}]`, file.blob, file.name);
  });
  return out;
}

const reply = (status, message) => new Response(message, { status });

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    if (url.pathname !== "/report" || request.method !== "POST") return reply(404, "not found");
    if (Number(request.headers.get("Content-Length") ?? 0) > MAX_TOTAL + 64 * 1024) {
      return reply(400, "report is too big");
    }
    const { success } = await env.LIMITER.limit({ key: request.headers.get("CF-Connecting-IP") ?? "unknown" });
    if (!success) return reply(429, "too many reports");

    let form;
    try {
      form = await request.formData();
    } catch {
      return reply(400, "not a multipart report");
    }
    const checked = await validateReport(form, request.headers);
    if (!checked.ok) return reply(400, checked.error);

    const sent = await fetch(env.DISCORD_WEBHOOK_URL, { method: "POST", body: discordForm(checked.report) });
    return sent.ok ? reply(200, "ok") : reply(502, "could not forward the report");
  },
};
