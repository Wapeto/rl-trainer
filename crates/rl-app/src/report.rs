//! Bug and idea reports: validated here with the same limits as the relay
//! (`tools/report-relay/worker.js`), then sent to the relay or saved to a folder.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use ts_rs::TS;

use crate::app::App;

pub const TEXT_MAX_CHARS: usize = 2000;
pub const CONTACT_MAX_CHARS: usize = 64;
pub const SCREENSHOT_MAX_BYTES: usize = 8 * 1024 * 1024;
/// The log tail attached to a report; the relay accepts up to 300 KB.
pub const LOG_TAIL_BYTES: u64 = 200 * 1024;
const SEND_TIMEOUT: Duration = Duration::from_secs(15);

/// The relay address baked in at build time; builds without it can only save reports.
pub fn relay_url() -> Option<&'static str> {
    option_env!("RL_REPORT_RELAY_URL").filter(|url| !url.is_empty())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Bug,
    Idea,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Kind::Bug => "bug",
            Kind::Idea => "idea",
        }
    }
}

#[derive(Debug, Clone)]
pub struct Report {
    pub kind: Kind,
    pub text: String,
    pub contact: String,
    pub screenshot: Option<Vec<u8>>,
    pub include_logs: bool,
}

/// What the Feedback page needs to know up front.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReportInfo {
    /// False in builds without a relay address: the page only offers saving.
    pub can_send: bool,
}

#[derive(Debug, Error)]
pub enum ReportError {
    #[error("{0}")]
    Invalid(String),
    #[error("Sending reports is not set up in this build. Save the report instead.")]
    NoRelay,
    #[error("Too many reports, try again in a minute.")]
    TooMany,
    #[error("The report was refused: {0}")]
    Refused(String),
    #[error("The report service had a problem ({0}). Try again later, or save the report.")]
    Server(u16),
    #[error("Couldn't reach the report service: {0}")]
    Network(String),
    #[error("Couldn't save the report: {0}")]
    Save(String),
}

fn image_type(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(&[0x89, b'P', b'N', b'G']) {
        Some("png")
    } else if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        Some("jpg")
    } else {
        None
    }
}

impl Report {
    pub fn validate(&self) -> Result<(), ReportError> {
        let invalid = |m: &str| Err(ReportError::Invalid(m.to_owned()));
        let len = self.text.trim().chars().count();
        if len == 0 {
            return invalid("Say what happened first.");
        }
        if len > TEXT_MAX_CHARS {
            return invalid("The text is too long (2000 characters at most).");
        }
        if self.contact.trim().chars().count() > CONTACT_MAX_CHARS {
            return invalid("The name is too long (64 characters at most).");
        }
        if let Some(shot) = &self.screenshot {
            if image_type(shot).is_none() {
                return invalid("The screenshot must be a PNG or JPEG image.");
            }
            if shot.len() > SCREENSHOT_MAX_BYTES {
                return invalid("The screenshot is bigger than 8 MB.");
            }
        }
        Ok(())
    }
}

/// Replaces `needle` (ASCII case-insensitive) with `with`.
fn replace_ci(text: &str, needle: &str, with: &str) -> String {
    if needle.is_empty() {
        return text.to_owned();
    }
    let (lower, needle) = (text.to_ascii_lowercase(), needle.to_ascii_lowercase());
    let mut out = String::with_capacity(text.len());
    let mut rest = 0;
    while let Some(at) = lower[rest..].find(&needle) {
        out.push_str(&text[rest..rest + at]);
        out.push_str(with);
        rest += at + needle.len();
    }
    out.push_str(&text[rest..]);
    out
}

/// Hides who the log came from: the profile folder (either slash) and the computer name.
pub fn redact(text: &str, user_profile: Option<&str>, computer: Option<&str>) -> String {
    let mut out = text.to_owned();
    if let Some(profile) = user_profile.filter(|p| !p.is_empty()) {
        for form in [
            profile.replace('\\', "/"),
            profile.replace('/', "\\"),
            profile.replace('/', "\\\\"),
        ] {
            out = replace_ci(&out, &form, "%USERPROFILE%");
        }
    }
    if let Some(name) = computer.filter(|n| n.len() > 2) {
        out = replace_ci(&out, name, "<computer>");
    }
    out
}

/// The last 200 KB of the newest log file, redacted. `None` when there is no log.
pub fn recent_log(dir: &Path) -> Option<String> {
    let newest = std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .max_by_key(|e| e.metadata().and_then(|m| m.modified()).ok())?;
    let mut file = std::fs::File::open(newest.path()).ok()?;
    let size = file.metadata().ok()?.len();
    file.seek(SeekFrom::Start(size.saturating_sub(LOG_TAIL_BYTES)))
        .ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    Some(redact(
        &text,
        std::env::var("USERPROFILE").ok().as_deref(),
        std::env::var("COMPUTERNAME").ok().as_deref(),
    ))
}

fn windows_version() -> String {
    sysinfo::System::long_os_version().unwrap_or_else(|| "unknown".to_owned())
}

/// POSTs the report (and `log`, when asked for) to the relay.
pub async fn send(
    report: &Report,
    log: Option<String>,
    url: &str,
    version: &str,
) -> Result<(), ReportError> {
    use reqwest::multipart::{Form, Part};
    report.validate()?;
    let mut form = Form::new()
        .text("kind", report.kind.as_str())
        .text("text", report.text.trim().to_owned())
        .text("contact", report.contact.trim().to_owned())
        .text("version", version.to_owned())
        .text("windows", windows_version());
    if let Some(shot) = &report.screenshot {
        let ext = image_type(shot).unwrap_or("png");
        let mime = if ext == "png" {
            "image/png"
        } else {
            "image/jpeg"
        };
        let part = Part::bytes(shot.clone())
            .file_name(format!("screenshot.{ext}"))
            .mime_str(mime)
            .map_err(|e| ReportError::Network(e.to_string()))?;
        form = form.part("screenshot", part);
    }
    if let Some(log) = log {
        let part = Part::bytes(log.into_bytes())
            .file_name("log.txt")
            .mime_str("text/plain")
            .map_err(|e| ReportError::Network(e.to_string()))?;
        form = form.part("log", part);
    }
    let client = reqwest::Client::builder()
        .timeout(SEND_TIMEOUT)
        .build()
        .map_err(|e| ReportError::Network(e.to_string()))?;
    let res = client
        .post(url)
        .header("X-RL-Trainer-Version", version)
        .multipart(form)
        .send()
        .await
        .map_err(|e| ReportError::Network(e.without_url().to_string()))?;
    let status = res.status();
    match status.as_u16() {
        200..=299 => Ok(()),
        429 => Err(ReportError::TooMany),
        400..=499 => Err(ReportError::Refused(
            res.text()
                .await
                .unwrap_or_default()
                .chars()
                .take(200)
                .collect(),
        )),
        code => Err(ReportError::Server(code)),
    }
}

/// Writes `<documents>\RL Trainer reports\<date-time>\` with `report.txt`, the screenshot and
/// `log.txt`; returns the folder.
pub fn save(
    report: &Report,
    log: Option<&str>,
    documents: &Path,
    version: &str,
) -> Result<PathBuf, ReportError> {
    report.validate()?;
    let stamp = chrono::Local::now().format("%Y-%m-%d_%H-%M-%S").to_string();
    let dir = documents.join("RL Trainer reports").join(stamp);
    let io = |e: std::io::Error| ReportError::Save(e.to_string());
    std::fs::create_dir_all(&dir).map_err(io)?;
    let body = format!(
        "Kind: {}\nVersion: {version}\nWindows: {}\nFrom: {}\n\n{}\n",
        report.kind.as_str(),
        windows_version(),
        report.contact.trim(),
        report.text.trim()
    );
    std::fs::write(dir.join("report.txt"), body).map_err(io)?;
    if let Some(shot) = &report.screenshot {
        let name = format!("screenshot.{}", image_type(shot).unwrap_or("png"));
        std::fs::write(dir.join(name), shot).map_err(io)?;
    }
    if let Some(log) = log {
        std::fs::write(dir.join("log.txt"), log).map_err(io)?;
    }
    Ok(dir)
}

/// What the Feedback page sends: the screenshot arrives base64-encoded.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReportArgs {
    pub kind: Kind,
    pub text: String,
    pub contact: String,
    pub screenshot: Option<String>,
    pub include_logs: bool,
}

impl TryFrom<ReportArgs> for Report {
    type Error = ReportError;

    fn try_from(args: ReportArgs) -> Result<Self, ReportError> {
        use base64::Engine;
        let screenshot = args
            .screenshot
            .map(|b64| base64::engine::general_purpose::STANDARD.decode(b64))
            .transpose()
            .map_err(|_| ReportError::Invalid("The screenshot could not be read.".into()))?;
        Ok(Report {
            kind: args.kind,
            text: args.text,
            contact: args.contact,
            screenshot,
            include_logs: args.include_logs,
        })
    }
}

impl App {
    pub fn report_info(&self) -> ReportInfo {
        ReportInfo {
            can_send: self.inner.report_url.is_some(),
        }
    }

    fn report_log(&self, report: &Report) -> Option<String> {
        report
            .include_logs
            .then(|| recent_log(&self.inner.paths.logs))
            .flatten()
    }

    pub async fn send_report(&self, report: Report) -> Result<(), ReportError> {
        let url = self
            .inner
            .report_url
            .as_deref()
            .ok_or(ReportError::NoRelay)?;
        let log = self.report_log(&report);
        send(&report, log, url, env!("CARGO_PKG_VERSION")).await
    }

    /// Saves the report under Documents and shows the folder; returns its path.
    pub fn save_report(&self, report: &Report) -> Result<PathBuf, ReportError> {
        let documents = self
            .inner
            .platform
            .documents_dir()
            .ok_or_else(|| ReportError::Save("the Documents folder was not found".into()))?;
        let log = self.report_log(report);
        let dir = save(
            report,
            log.as_deref(),
            &documents,
            env!("CARGO_PKG_VERSION"),
        )?;
        if let Err(err) = self.inner.platform.open_folder(&dir) {
            tracing::warn!(error = %err, "could not show the saved report");
        }
        Ok(dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = &[0x89, b'P', b'N', b'G', 0, 0];

    fn report(text: &str) -> Report {
        Report {
            kind: Kind::Bug,
            text: text.into(),
            contact: String::new(),
            screenshot: None,
            include_logs: true,
        }
    }

    #[test]
    fn validation_uses_the_relay_limits() {
        assert!(report("it broke").validate().is_ok());
        assert!(report("   ").validate().is_err());
        assert!(report(&"x".repeat(2001)).validate().is_err());
        let mut r = report("ok");
        r.contact = "n".repeat(65);
        assert!(r.validate().is_err());
        r.contact.clear();
        r.screenshot = Some(PNG.to_vec());
        assert!(r.validate().is_ok());
        r.screenshot = Some(b"GIF89a".to_vec());
        assert!(r.validate().is_err(), "not a PNG or JPEG");
        r.screenshot = Some([PNG, &vec![0; SCREENSHOT_MAX_BYTES]].concat());
        assert!(r.validate().is_err(), "over 8 MB");
    }

    #[test]
    fn redaction_hides_the_profile_path_and_computer_name() {
        let text = r"open C:\Users\Sam\AppData\x and C:/users/sam/y on SAM-PC (sam-pc)";
        let out = redact(text, Some(r"C:\Users\Sam"), Some("SAM-PC"));
        assert_eq!(
            out,
            r"open %USERPROFILE%\AppData\x and %USERPROFILE%/y on <computer> (<computer>)"
        );
        assert_eq!(redact("nothing here", None, None), "nothing here");
    }

    #[test]
    fn the_log_is_the_redacted_tail_of_the_newest_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("old.log"), "old").unwrap();
        std::thread::sleep(Duration::from_millis(30));
        let big = format!("HEAD{}TAIL", "a".repeat(LOG_TAIL_BYTES as usize));
        std::fs::write(dir.path().join("new.log"), &big).unwrap();
        let log = recent_log(dir.path()).unwrap();
        assert_eq!(log.len(), LOG_TAIL_BYTES as usize);
        assert!(log.ends_with("TAIL") && !log.contains("HEAD"));
        assert!(recent_log(&dir.path().join("missing")).is_none());
    }

    #[test]
    fn saving_writes_the_report_screenshot_and_log() {
        let docs = tempfile::tempdir().unwrap();
        let mut r = report("the radar is empty");
        r.contact = "Sam".into();
        r.screenshot = Some(PNG.to_vec());
        let dir = save(&r, Some("log text"), docs.path(), "0.2.0").unwrap();
        assert!(dir.starts_with(docs.path().join("RL Trainer reports")));
        let text = std::fs::read_to_string(dir.join("report.txt")).unwrap();
        assert!(
            text.contains("Kind: bug")
                && text.contains("From: Sam")
                && text.contains("the radar is empty")
        );
        assert_eq!(std::fs::read(dir.join("screenshot.png")).unwrap(), PNG);
        assert_eq!(
            std::fs::read_to_string(dir.join("log.txt")).unwrap(),
            "log text"
        );
    }
}
