//! The evening recording with the phone actions a player would take: pick a pack after the
//! reset drills, then mark the last (skipped) shot as SKIP.

mod common;

use std::path::PathBuf;

use rl_core::facts::Fact;
use rl_core::fixture::open_spike_log;
use rl_core::ingest::IngestMsg;
use rl_core::pipeline::Pipeline;
use rl_core::store::{list_sessions, open_in_memory};
use rl_core::tracker::TrackerConfig;

#[test]
fn pack_change_mid_training_splits_and_skip_fixes_the_make_rate() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/training-ranked-2026-10-07.jsonl.gz");
    let events = open_spike_log(&path).unwrap();
    let last = events.last().unwrap().at_ms;
    let mut p = Pipeline::new(open_in_memory().unwrap(), TrackerConfig::default()).unwrap();
    let mut finished = 0;
    let mut split_done = false;
    for event in events {
        let at = event.at_ms;
        let facts = p.handle(&IngestMsg::Event(event)).unwrap();
        finished += facts
            .iter()
            .filter(|f| matches!(f, Fact::AttemptFinished(_)))
            .count();
        if finished == 6 && !split_done {
            let block = p.open_block_id().unwrap();
            p.set_block_pack(block, Some(1), at).unwrap();
            split_done = true;
        }
    }
    p.game_exited(last).unwrap();

    let conn = p.recorder().conn();
    let last_attempt: i64 = conn
        .query_row(
            "SELECT id FROM training_attempt ORDER BY started_at DESC, id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .unwrap();
    // The session has ended, so the phone could no longer skip; edit through SQL the way the
    // desktop history editor will (Plan 3) to check the make-rate rule.
    conn.execute(
        "UPDATE training_attempt SET skipped = 1 WHERE id = ?1",
        [last_attempt],
    )
    .unwrap();

    let session = &list_sessions(conn).unwrap()[0];
    let training: Vec<_> = session
        .blocks
        .iter()
        .filter(|b| b.kind == "training")
        .collect();
    assert_eq!(training.len(), 2);
    assert_eq!((training[0].attempts, training[0].makes), (6, 0));
    assert_eq!((training[1].attempts, training[1].makes), (5, 4));
    assert_eq!(training[1].make_rate(), Some(0.8));
    let packs: Vec<Option<i64>> = conn
        .prepare("SELECT pack_id FROM block WHERE kind = 'training' ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(packs, [None, Some(1)]);
}
