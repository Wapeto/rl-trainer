//! End-to-end: the anonymized 2026-10-07 afternoon recording (free play → custom training →
//! casual 2v2 → saved replay).

mod common;

use common::{attempts, replay, strings};
use rl_core::facts::Fact;
use rl_core::pipeline::Pipeline;

const FIXTURE: &str = "spike-2026-10-07.jsonl.gz";

fn run() -> (Pipeline, Vec<Fact>) {
    replay(FIXTURE)
}

#[test]
fn detects_the_profile_once() {
    let (_, facts) = run();
    let profiles: Vec<_> = facts
        .iter()
        .filter_map(|f| match f {
            Fact::ProfileDetected {
                primary_id,
                display_name,
                ..
            } => Some((primary_id.as_str(), display_name.as_str())),
            _ => None,
        })
        .collect();
    assert_eq!(profiles, [("Epic|me|0", "Me")]);
}

#[test]
fn splits_into_one_session_with_four_blocks() {
    let (p, _) = run();
    let conn = p.recorder().conn();
    let sessions: i64 = conn
        .query_row("SELECT COUNT(*) FROM session", [], |r| r.get(0))
        .unwrap();
    assert_eq!(sessions, 1);
    assert_eq!(
        strings(conn, "SELECT kind FROM block ORDER BY started_at, id"),
        ["freeplay", "training", "match", "other"]
    );
}

#[test]
fn classifies_training_attempts_including_resets() {
    let (p, _) = run();
    let rows = attempts(p.recorder().conn());
    let expected = [
        ("make", false),
        ("miss", true),
        ("make", false),
        ("make", false),
        ("make", true),
        ("miss", false),
        ("make", false),
        ("make", false),
        ("make", false),
    ];
    let rows: Vec<(&str, bool)> = rows.iter().map(|(o, r)| (o.as_str(), *r)).collect();
    assert_eq!(rows, expected);
}

#[test]
fn aggregates_free_play() {
    let (p, _) = run();
    let (touches, hard, goals, crossbar, max): (i64, i64, i64, i64, f64) = p
        .recorder()
        .conn()
        .query_row(
            "SELECT touches, hard_hits, goals, crossbar_hits, max_hit_speed FROM freeplay_stats",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
        )
        .unwrap();
    assert_eq!((touches, hard, goals, crossbar), (53, 18, 7, 6));
    assert!((max - 112.6809).abs() < 0.001);
}

#[test]
fn records_the_casual_match_without_replay_duplicates() {
    let (p, _) = run();
    let conn = p.recorder().conn();
    let n: i64 = conn
        .query_row("SELECT COUNT(*) FROM match_record", [], |r| r.get(0))
        .unwrap();
    assert_eq!(n, 1);
    // Tuples above 12 elements don't implement PartialEq, so the row is checked in two parts.
    let outcome: (String, i64, i64, bool, bool, i64, bool) = conn
        .query_row(
            "SELECT result, team_score, opponent_score, overtime, ranked, team_size, metrics_partial
             FROM match_record",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(outcome, ("loss".to_string(), 2, 3, true, false, 2, false));
    let mine: (i64, i64, i64, i64, i64, i64, i64) = conn
        .query_row(
            "SELECT goals, shots, saves, assists, score, touches, epic_saves FROM match_record",
            [],
            |r| {
                Ok((
                    r.get(0)?,
                    r.get(1)?,
                    r.get(2)?,
                    r.get(3)?,
                    r.get(4)?,
                    r.get(5)?,
                    r.get(6)?,
                ))
            },
        )
        .unwrap();
    assert_eq!(mine, (2, 3, 2, 0, 617, 47, 1));
}
