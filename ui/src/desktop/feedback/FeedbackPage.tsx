import { useState, type ClipboardEvent } from "react";
import { sentence } from "../act";
import { errorText, type ReportInput } from "../api";
import { useLoad } from "../components/useLoad";
import type { PageProps } from "../pageProps";

export const TEXT_MAX = 2000;
export const CONTACT_MAX = 64;
export const SCREENSHOT_MAX_BYTES = 8 * 1024 * 1024;

const IMAGE_TYPES = ["image/png", "image/jpeg"];

function readDataUrl(file: Blob): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader();
    reader.onload = () => resolve(String(reader.result));
    reader.onerror = () => reject(new Error("The screenshot could not be read."));
    reader.readAsDataURL(file);
  });
}

export function FeedbackPage({ api, notify }: PageProps) {
  const infoLoad = useLoad(() => api.reportInfo(), "info");
  const info = infoLoad.data;
  const [kind, setKind] = useState<ReportInput["kind"]>("bug");
  const [text, setText] = useState("");
  const [contact, setContact] = useState("");
  const [logs, setLogs] = useState(true);
  const [shot, setShot] = useState<string | null>(null); // a data: URL
  const [problem, setProblem] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const [busy, setBusy] = useState(false);

  async function attach(file: Blob | null | undefined) {
    if (!file) return;
    if (!IMAGE_TYPES.includes(file.type)) return setProblem("The screenshot must be a PNG or JPEG image.");
    if (file.size > SCREENSHOT_MAX_BYTES) return setProblem("The screenshot is bigger than 8 MB.");
    try {
      setShot(await readDataUrl(file));
      setProblem(null);
    } catch (err) {
      setProblem(errorText(err));
    }
  }
  function onPaste(e: ClipboardEvent) {
    const image = [...e.clipboardData.files].find((f) => IMAGE_TYPES.includes(f.type));
    if (image) {
      e.preventDefault();
      void attach(image);
    }
  }

  function build(): ReportInput | null {
    if (!text.trim()) {
      setProblem("Say what happened first.");
      return null;
    }
    setProblem(null);
    return { kind, text: text.trim(), contact: contact.trim(), screenshot: shot?.split(",")[1] ?? null, include_logs: logs };
  }
  async function run(action: (r: ReportInput) => Promise<unknown>, okText: (result: unknown) => string, isSend: boolean) {
    const report = build();
    if (!report) return;
    setBusy(true);
    try {
      const result = await action(report);
      notify(okText(result), "ok");
      setText("");
      setShot(null);
      setFailed(false);
    } catch (err) {
      setProblem(sentence(errorText(err)));
      setFailed(isSend);
    } finally {
      setBusy(false);
    }
  }

  const canSend = info?.can_send ?? false;
  const sends = ["your text", "the app version and Windows version"]
    .concat(
      contact.trim() ? ["your name"] : [],
      shot ? ["your screenshot"] : [],
      logs ? ["recent log lines (no personal paths)"] : [],
    )
    .join(", ");
  return (
    <main className="d-main" onPaste={onPaste}>
      <div className="d-settings">
        <div className="d-hd">
          <h1>Feedback</h1>
        </div>
        <section className="d-card glass d-form" aria-label="Report">
          <div className="d-btns" role="group" aria-label="Kind">
            {(["bug", "idea"] as const).map((k) => (
              <button
                key={k}
                type="button"
                className={`d-btn${kind === k ? " d-btn--primary" : ""}`}
                aria-pressed={kind === k}
                onClick={() => setKind(k)}
              >
                {k === "bug" ? "Bug" : "Idea"}
              </button>
            ))}
          </div>
          <label>
            What happened?
            <textarea
              className="d-input"
              value={text}
              maxLength={TEXT_MAX}
              onChange={(e) => setText(e.target.value)}
              placeholder={kind === "bug" ? "What did you do, and what went wrong?" : "What would make this better?"}
            />
          </label>
          <span className="d-m">{`${text.length} / ${TEXT_MAX}`}</span>
          <label>
            Your name (optional, so I know who sent it)
            <input className="d-input" value={contact} maxLength={CONTACT_MAX} onChange={(e) => setContact(e.target.value)} />
          </label>
          <div>
            <label className="d-btn">
              Add a screenshot
              <input
                type="file"
                accept="image/png,image/jpeg"
                hidden
                aria-label="Screenshot file"
                onChange={(e) => void attach(e.target.files?.[0])}
              />
            </label>
            <span className="d-m"> or paste one with Ctrl+V</span>
            {shot && (
              <div className="d-shot">
                <img src={shot} alt="Screenshot preview" />
                <button type="button" className="d-link" onClick={() => setShot(null)}>
                  Remove screenshot
                </button>
              </div>
            )}
          </div>
          <label className="d-check">
            <input type="checkbox" checked={logs} onChange={(e) => setLogs(e.target.checked)} /> Include recent logs
          </label>
          <p className="d-hint">{`What gets sent: ${sends}.`}</p>
          {problem && (
            <p className="d-err" role="alert">
              {problem}
            </p>
          )}
          {infoLoad.error && <p className="d-hint">Couldn't check whether sending works. You can still save the report.</p>}
          {!canSend && info && (
            <p className="d-hint">Sending is not set up in this build. Save the report, then send me the files in its folder.</p>
          )}
          <div className="d-btns">
            <button
              type="button"
              className={`d-btn${canSend && !failed ? " d-btn--primary" : ""}`}
              disabled={busy || !canSend}
              onClick={() => run((r) => api.sendReport(r), () => "Thanks, sent", true)}
            >
              Send
            </button>
            <button
              type="button"
              className={`d-btn${failed || !canSend ? " d-btn--primary" : ""}`}
              disabled={busy}
              onClick={() => run((r) => api.saveReport(r), (dir) => `Saved to ${String(dir)}`, false)}
            >
              Save report
            </button>
          </div>
        </section>
      </div>
    </main>
  );
}
