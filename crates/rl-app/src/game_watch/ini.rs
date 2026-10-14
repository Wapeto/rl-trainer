//! `TAGame\Config\DefaultStatsAPI.ini`: whether the Stats API is on, and the one-line fix.

use std::io;
use std::path::{Path, PathBuf};

use serde::Serialize;
use ts_rs::TS;

pub const SECTION: &str = "[TAGame.MatchStatsExporter_TA]";
pub const DEFAULT_PORT: u16 = 49123;
pub const FIXED_RATE: u32 = 10;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ExporterValues {
    pub packet_send_rate: Option<f64>,
    pub port: Option<u16>,
    pub web_port: Option<u16>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum StatsApiStatus {
    #[default]
    Unknown,
    Ok {
        rate: f64,
        port: u16,
        web_port: Option<u16>,
    },
    Disabled {
        #[ts(type = "string")]
        ini: PathBuf,
    },
    FileMissing {
        #[ts(type = "string")]
        ini: PathBuf,
    },
    Unreadable {
        #[ts(type = "string")]
        ini: PathBuf,
        error: String,
    },
    InstallNotFound,
}

impl StatsApiStatus {
    pub fn port(&self) -> Option<u16> {
        match self {
            Self::Ok { port, .. } => Some(*port),
            _ => None,
        }
    }
}

fn clean(line: &str) -> &str {
    line.trim_start_matches('\u{feff}').trim()
}

fn key_value(line: &str) -> Option<(&str, &str)> {
    let line = clean(line);
    if line.starts_with(';') || line.starts_with('#') {
        return None;
    }
    line.split_once('=').map(|(k, v)| (k.trim(), v.trim()))
}

fn is_header(line: &str) -> bool {
    clean(line).starts_with('[')
}

fn is_our_section(line: &str) -> bool {
    clean(line).eq_ignore_ascii_case(SECTION)
}

pub fn parse(text: &str) -> ExporterValues {
    let mut values = ExporterValues::default();
    let mut in_section = false;
    for line in text.lines() {
        if is_header(line) {
            in_section = is_our_section(line);
            continue;
        }
        if !in_section {
            continue;
        }
        if let Some((key, value)) = key_value(line) {
            match key.to_ascii_lowercase().as_str() {
                "packetsendrate" => values.packet_send_rate = value.parse().ok(),
                "port" => values.port = value.parse().ok(),
                "webport" => values.web_port = value.parse().ok(),
                _ => {}
            }
        }
    }
    values
}

pub fn evaluate(ini: &Path, read: io::Result<String>) -> StatsApiStatus {
    let text = match read {
        Ok(text) => text,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return StatsApiStatus::FileMissing {
                ini: ini.to_path_buf(),
            }
        }
        Err(e) => {
            return StatsApiStatus::Unreadable {
                ini: ini.to_path_buf(),
                error: e.to_string(),
            }
        }
    };
    let v = parse(&text);
    let port = v.port.unwrap_or(DEFAULT_PORT);
    match v.packet_send_rate {
        Some(rate) if rate > 0.0 && port != 0 => StatsApiStatus::Ok {
            rate,
            port,
            web_port: v.web_port,
        },
        _ => StatsApiStatus::Disabled {
            ini: ini.to_path_buf(),
        },
    }
}

pub fn check(ini: &Path) -> StatsApiStatus {
    evaluate(ini, std::fs::read_to_string(ini))
}

fn find_key(lines: &[String], start: usize, end: usize, key: &str) -> Option<usize> {
    (start + 1..end)
        .find(|&i| key_value(&lines[i]).is_some_and(|(k, _)| k.eq_ignore_ascii_case(key)))
}

/// Sets `PacketSendRate=10` (and `Port=49123` when it is missing or 0) inside the exporter
/// section, adding the section if needed. Every other line is kept byte-for-byte.
pub fn fix(text: &str) -> String {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let trailing_newline = text.is_empty() || text.ends_with('\n');
    let mut lines: Vec<String> = text.lines().map(str::to_owned).collect();
    let start = match lines.iter().position(|l| is_our_section(l)) {
        Some(start) => start,
        None => {
            if lines.last().is_some_and(|l| !l.trim().is_empty()) {
                lines.push(String::new());
            }
            lines.push(SECTION.to_owned());
            lines.len() - 1
        }
    };
    let mut end = lines[start + 1..]
        .iter()
        .position(|l| is_header(l))
        .map_or(lines.len(), |i| start + 1 + i);
    let rate_line = format!("PacketSendRate={FIXED_RATE}");
    match find_key(&lines, start, end, "PacketSendRate") {
        Some(i) => lines[i] = rate_line,
        None => {
            lines.insert(end, rate_line);
            end += 1;
        }
    }
    let port_ok = find_key(&lines, start, end, "Port")
        .and_then(|i| key_value(&lines[i]).and_then(|(_, v)| v.parse::<u16>().ok()))
        .is_some_and(|p| p != 0);
    if !port_ok {
        let port_line = format!("Port={DEFAULT_PORT}");
        match find_key(&lines, start, end, "Port") {
            Some(i) => lines[i] = port_line,
            None => lines.insert(start + 1, port_line),
        }
    }
    let mut out = lines.join(newline);
    if trailing_newline {
        out.push_str(newline);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const REAL: &str = "[TAGame.MatchStatsExporter_TA]\r\n\r\n; Port the client will listen for tcp connections on (must be different than WebPort, set to 0 to disable)\r\nPort=49123\r\n\r\n; Port the client will listen for web connections on (must be different than Port, set to 0 to disable)\r\nWebPort=49124\r\n\r\n; How many times per second the game sends the update state (capped at 120, 0 disables this feature)\r\nPacketSendRate=10\r\n";

    fn ini() -> PathBuf {
        PathBuf::from("C:/RL/TAGame/Config/DefaultStatsAPI.ini")
    }

    #[test]
    fn parses_the_real_file() {
        let v = parse(REAL);
        assert_eq!(
            (v.packet_send_rate, v.port, v.web_port),
            (Some(10.0), Some(49123), Some(49124))
        );
        assert_eq!(
            evaluate(&ini(), Ok(REAL.into())),
            StatsApiStatus::Ok {
                rate: 10.0,
                port: 49123,
                web_port: Some(49124)
            }
        );
    }

    #[test]
    fn zero_rate_or_zero_port_is_disabled_and_errors_are_reported() {
        let off = REAL.replace("PacketSendRate=10", "PacketSendRate=0");
        assert_eq!(
            evaluate(&ini(), Ok(off)),
            StatsApiStatus::Disabled { ini: ini() }
        );
        let no_port = REAL.replace("Port=49123", "Port=0");
        assert_eq!(
            evaluate(&ini(), Ok(no_port)),
            StatsApiStatus::Disabled { ini: ini() }
        );
        let missing = std::io::Error::from(std::io::ErrorKind::NotFound);
        assert_eq!(
            evaluate(&ini(), Err(missing)),
            StatsApiStatus::FileMissing { ini: ini() }
        );
        let denied = std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        assert!(matches!(
            evaluate(&ini(), Err(denied)),
            StatsApiStatus::Unreadable { .. }
        ));
    }

    #[test]
    fn keys_outside_the_section_are_ignored() {
        let text = "[Other]\nPacketSendRate=30\n[TAGame.MatchStatsExporter_TA]\nPacketSendRate=0\n";
        assert_eq!(parse(text).packet_send_rate, Some(0.0));
    }

    #[test]
    fn fix_sets_the_rate_and_leaves_every_other_line_identical() {
        let off = REAL.replace("PacketSendRate=10", "PacketSendRate=0");
        assert_eq!(fix(&off), REAL);
        let no_port = off.replace("Port=49123", "Port=0");
        assert_eq!(fix(&no_port), REAL);
    }

    #[test]
    fn fix_adds_missing_keys_and_sections() {
        let fixed = fix("[TAGame.MatchStatsExporter_TA]\nWebPort=49124\n");
        assert_eq!(parse(&fixed).packet_send_rate, Some(10.0));
        assert_eq!(parse(&fixed).port, Some(49123));
        assert!(fixed.contains("WebPort=49124\n"));
        let empty = fix("");
        assert!(
            matches!(evaluate(&ini(), Ok(empty)), StatsApiStatus::Ok { rate, .. } if rate == 10.0)
        );
    }
}
