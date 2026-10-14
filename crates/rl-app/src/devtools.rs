//! A stand-in for the game's Stats API socket, for tests and `rl-replay serve --recording`.

use std::fs::File;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use flate2::read::GzDecoder;
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::task::JoinHandle;
use tokio::time::sleep;

type Script = Arc<Vec<(Duration, String)>>;
type Idle = Arc<Option<(Duration, String)>>;

pub struct FakeStatsApi {
    pub addr: String,
    task: JoinHandle<()>,
}

impl FakeStatsApi {
    /// Each connection receives `script` (waiting each delay before its message), then `idle`
    /// repeatedly if given; otherwise the connection stays open until the client leaves.
    pub async fn start(
        script: Vec<(Duration, String)>,
        idle: Option<(Duration, String)>,
    ) -> io::Result<Self> {
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let addr = listener.local_addr()?.to_string();
        let script: Script = Arc::new(script);
        let idle: Idle = Arc::new(idle);
        let task = tokio::spawn(async move {
            while let Ok((socket, _)) = listener.accept().await {
                tokio::spawn(serve_one(socket, script.clone(), idle.clone()));
            }
        });
        Ok(Self { addr, task })
    }
}

impl Drop for FakeStatsApi {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn serve_one(mut socket: TcpStream, script: Script, idle: Idle) {
    for (delay, message) in script.iter() {
        sleep(*delay).await;
        if socket.write_all(message.as_bytes()).await.is_err() {
            return;
        }
    }
    match idle.as_ref() {
        Some((every, message)) => loop {
            sleep(*every).await;
            if socket.write_all(message.as_bytes()).await.is_err() {
                return;
            }
        },
        None => {
            let mut buf = [0_u8; 1];
            let _ = socket.read(&mut buf).await; // returns when the client disconnects
        }
    }
}

pub fn update_state_json(playlist_id: i32, time_seconds: f64) -> String {
    json!({
        "Event": "UpdateState",
        "Data": {
            "MatchGuid": "",
            "Players": [{"Name": "Me", "PrimaryId": "Epic|me|0", "Shortcut": 0, "TeamNum": 0, "Speed": 40.0}],
            "Game": {
                "PlaylistId": playlist_id,
                "TimeSeconds": time_seconds,
                "bHasTarget": true,
                "Target": {"Name": "Me", "Shortcut": 0, "TeamNum": 0},
                "Teams": [{"Name": "Blue", "TeamNum": 0, "Score": 0}]
            }
        }
    })
    .to_string()
}

pub fn event_json(name: &str) -> String {
    json!({"Event": name, "Data": {"MatchGuid": ""}}).to_string()
}

pub fn goal_json(scorer: &str) -> String {
    json!({
        "Event": "GoalScored",
        "Data": {"MatchGuid": "", "GoalSpeed": 80.0, "Scorer": {"Name": scorer, "Shortcut": 0, "TeamNum": 0}}
    })
    .to_string()
}

/// Turns a probe.mjs recording (`{t, event, data}` lines, optionally .gz) into a script for
/// `FakeStatsApi`, played `speed` times faster than recorded.
pub fn recording_script(path: &Path, speed: f64) -> io::Result<Vec<(Duration, String)>> {
    let file = File::open(path)?;
    let reader: Box<dyn BufRead> = if path.extension().is_some_and(|e| e == "gz") {
        Box::new(BufReader::new(GzDecoder::new(file)))
    } else {
        Box::new(BufReader::new(file))
    };
    let speed = if speed.is_finite() && speed > 0.0 {
        speed
    } else {
        1.0
    };
    let mut script = Vec::new();
    let mut previous: Option<i64> = None;
    for line in reader.lines() {
        let line = line?;
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        let (Some(t), Some(event)) = (value["t"].as_i64(), value["event"].as_str()) else {
            continue; // markers and other non-event lines
        };
        let gap = previous.map_or(0, |p| (t - p).max(0));
        previous = Some(t);
        let delay = Duration::from_millis((gap as f64 / speed) as u64);
        script.push((
            delay,
            json!({"Event": event, "Data": value["data"]}).to_string(),
        ));
    }
    Ok(script)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn recordings_become_timed_stats_api_messages() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../rl-core/tests/fixtures/spike-2026-10-07.jsonl.gz");
        let script = recording_script(&path, 4.0).unwrap();
        let events = rl_core::fixture::open_spike_log(&path).unwrap();
        assert_eq!(script.len(), events.len());
        assert_eq!(script[0].0, Duration::ZERO);
        let first: serde_json::Value = serde_json::from_str(&script[0].1).unwrap();
        assert!(first["Event"].is_string());
        let total: Duration = script.iter().map(|(d, _)| *d).sum();
        let span = Duration::from_millis((events.last().unwrap().at_ms - events[0].at_ms) as u64);
        assert!(total <= span / 3, "played at 4x speed");
    }
}
