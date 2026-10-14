//! Shared helpers for the recording-based integration tests.
// Each test binary includes this module and uses a different subset of it.
#![allow(dead_code)]

use std::path::PathBuf;

use rl_core::facts::Fact;
use rl_core::fixture::open_spike_log;
use rl_core::ingest::IngestMsg;
use rl_core::pipeline::Pipeline;
use rl_core::store::open_in_memory;
use rl_core::tracker::TrackerConfig;
use rusqlite::Connection;

/// Replays `tests/fixtures/<name>` through the full pipeline into an in-memory database.
pub fn replay(name: &str) -> (Pipeline, Vec<Fact>) {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    let events = open_spike_log(&path).expect("fixture is readable");
    let last = events.last().expect("fixture is not empty").at_ms;
    let mut pipeline = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
    let mut facts = Vec::new();
    for event in events {
        facts.extend(pipeline.handle(&IngestMsg::Event(event)).unwrap());
    }
    facts.extend(pipeline.game_exited(last).unwrap());
    (pipeline, facts)
}

pub fn strings(conn: &Connection, sql: &str) -> Vec<String> {
    let mut stmt = conn.prepare(sql).unwrap();
    stmt.query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// `(outcome, retry_after_make)` for every recorded attempt, in order.
pub fn attempts(conn: &Connection) -> Vec<(String, bool)> {
    let mut stmt = conn
        .prepare("SELECT outcome, retry_after_make FROM training_attempt ORDER BY id")
        .unwrap();
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}
