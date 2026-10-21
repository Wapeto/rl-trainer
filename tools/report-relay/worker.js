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

const THREAD_NAME_MAX = 100; // Discord's limit for a forum post title

/** The forum post title: kind and the start of the text's first line. */
export function threadName(report) {
  const head = `${report.kind === "bug" ? "Bug" : "Idea"}: `;
  const line = report.text.split("\n")[0].trim();
  const room = THREAD_NAME_MAX - head.length;
  return head + (line.length > room ? `${line.slice(0, room - 1)}…` : line);
}

/** The Discord webhook body for a forum channel: a new post with an embed, plus the files. */
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
  out.set("payload_json", JSON.stringify({ thread_name: threadName(report), embeds: [embed] }));
  [report.screenshot, report.log].filter(Boolean).forEach((file, n) => {
    out.set(`files[${n}]`, file.blob, file.name);
  });
  return out;
}

const reply = (status, message) => new Response(message, { status });

const ID = /^[0-9a-f]{32}$/;
const MAX_PING_VERSION = 20;
const MAX_PING_BODY = 1024;
const INSTALL_TTL_S = 30 * 24 * 60 * 60; // an install counts for 30 days after its last ping

/** Checks an install ping: { id, version } as JSON plus the version header. */
export function validatePing(body, headers) {
  const bad = (error) => ({ ok: false, error });
  if (!headers.get("X-RL-Trainer-Version")) return bad("missing version header");
  let data;
  try {
    data = JSON.parse(body);
  } catch {
    return bad("not JSON");
  }
  if (typeof data?.id !== "string" || !ID.test(data.id)) return bad("id must be 32 hex characters");
  if (typeof data.version !== "string" || data.version.length < 1 || data.version.length > MAX_PING_VERSION) {
    return bad("version must be 1 to 20 characters");
  }
  return { ok: true, ping: { id: data.id, version: data.version } };
}

async function handlePing(request, env) {
  const body = await request.text();
  if (body.length > MAX_PING_BODY) return reply(400, "ping is too big");
  const checked = validatePing(body, request.headers);
  if (!checked.ok) return reply(400, checked.error);
  await env.INSTALLS.put(checked.ping.id, checked.ping.version, { expirationTtl: INSTALL_TTL_S });
  return new Response(null, { status: 204 });
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);
    const isPing = url.pathname === "/ping";
    if ((url.pathname !== "/report" && !isPing) || request.method !== "POST") return reply(404, "not found");
    if (Number(request.headers.get("Content-Length") ?? 0) > MAX_TOTAL + 64 * 1024) {
      return reply(400, "report is too big");
    }
    const { success } = await env.LIMITER.limit({ key: request.headers.get("CF-Connecting-IP") ?? "unknown" });
    if (!success) return reply(429, "too many reports");
    if (isPing) return handlePing(request, env);

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
