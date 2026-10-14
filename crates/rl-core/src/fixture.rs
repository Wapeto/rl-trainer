//! Reads recordings written by spikes/stats-api/probe.mjs (`{t, event, data}` per line).

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use flate2::read::GzDecoder;
use serde_json::Value;
use thiserror::Error;

use crate::events::{decode_parts, DecodeError, TimedEvent};

#[derive(Debug, Error)]
pub enum FixtureError {
    #[error("cannot read log: {0}")]
    Io(#[from] std::io::Error),
    #[error("line {line}: {source}")]
    Line { line: usize, source: DecodeError },
    #[error("line {line}: missing field `{field}`")]
    MissingField { line: usize, field: &'static str },
}

pub fn open_spike_log(path: &Path) -> Result<Vec<TimedEvent>, FixtureError> {
    let file = File::open(path)?;
    if path.extension().is_some_and(|ext| ext == "gz") {
        read_spike_log(BufReader::new(GzDecoder::new(file)))
    } else {
        read_spike_log(BufReader::new(file))
    }
}

pub fn read_spike_log<R: BufRead>(reader: R) -> Result<Vec<TimedEvent>, FixtureError> {
    let mut events = Vec::new();
    for (index, line) in reader.lines().enumerate() {
        let line_no = index + 1;
        let line = line?;
        if line.trim().is_empty() {
            continue;
        }
        let value: Value = serde_json::from_str(&line).map_err(|e| FixtureError::Line {
            line: line_no,
            source: e.into(),
        })?;
        if value.get("marker").is_some() {
            continue;
        }
        let at_ms = value
            .get("t")
            .and_then(Value::as_i64)
            .ok_or(FixtureError::MissingField {
                line: line_no,
                field: "t",
            })?;
        let name =
            value
                .get("event")
                .and_then(Value::as_str)
                .ok_or(FixtureError::MissingField {
                    line: line_no,
                    field: "event",
                })?;
        let data = value.get("data").cloned().unwrap_or(Value::Null);
        let event = decode_parts(name, data).map_err(|source| FixtureError::Line {
            line: line_no,
            source,
        })?;
        events.push(TimedEvent { at_ms, event });
    }
    Ok(events)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::GameEvent;

    #[test]
    fn reads_events_and_skips_markers() {
        let log = r#"{"t":1,"marker":"freeplay"}
{"t":2,"phase":"freeplay","event":"RoundStarted","data":{"MatchGuid":""}}

{"t":3,"event":"MatchEnded","data":{"WinnerTeamNum":0}}
"#;
        let events = read_spike_log(log.as_bytes()).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(
            (events[0].at_ms, &events[0].event),
            (2, &GameEvent::RoundStarted)
        );
    }

    #[test]
    fn reports_the_failing_line() {
        let log =
            "{\"t\":1,\"event\":\"RoundStarted\",\"data\":{}}\n{\"event\":\"RoundStarted\"}\n";
        let err = read_spike_log(log.as_bytes()).unwrap_err();
        assert!(matches!(
            err,
            FixtureError::MissingField {
                line: 2,
                field: "t"
            }
        ));
    }
}
