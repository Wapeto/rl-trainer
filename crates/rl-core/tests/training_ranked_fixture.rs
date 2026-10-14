//! End-to-end: the anonymized 2026-10-07 evening recording. The player labelled each phase:
//! 3 shots left to time out, 3 resets after a miss, 1 timeout + 4 retries after a make,
//! skipped shots, then one ranked 1v1 (8-2 win) and one ranked 2v2 (2-3 loss).

mod common;

use common::{attempts, replay, strings};
use rl_core::pipeline::Pipeline;

const FIXTURE: &str = "training-ranked-2026-10-07.jsonl.gz";

type Row7 = (i64, i64, i64, i64, i64, i64, i64);

fn run() -> Pipeline {
    replay(FIXTURE).0
}

#[test]
fn labelled_training_phases_are_classified() {
    let p = run();
    let rows = attempts(p.recorder().conn());
    let rows: Vec<(&str, bool)> = rows.iter().map(|(o, r)| (o.as_str(), *r)).collect();
    let expected = [
        ("miss", false), // timeout
        ("miss", false),
        ("miss", false),
        ("miss", false), // reset after a miss: a miss
        ("miss", false),
        ("miss", false),
        ("miss", false), // retry-make phase: first shot timed out
        ("make", false),
        ("make", true), // retried after scoring
        ("make", true),
        ("make", true),
        ("miss", true), // the one skipped shot that had already started: a miss
    ];
    assert_eq!(rows, expected);
}

#[test]
fn splits_training_and_two_ranked_matches() {
    let p = run();
    assert_eq!(
        strings(
            p.recorder().conn(),
            "SELECT kind FROM block ORDER BY started_at, id"
        ),
        ["training", "match", "match"]
    );
}

#[test]
fn scoreboards_keep_players_who_left_before_the_end() {
    let p = run();
    let boards = strings(
        p.recorder().conn(),
        "SELECT scoreboard_json FROM match_record ORDER BY id",
    );
    let sizes: Vec<usize> = boards
        .iter()
        .map(|b| {
            serde_json::from_str::<Vec<serde_json::Value>>(b)
                .unwrap()
                .len()
        })
        .collect();
    assert_eq!(sizes, [2, 4]);
}

#[test]
fn records_ranked_duel_and_doubles() {
    let p = run();
    let mut stmt = p
        .recorder()
        .conn()
        .prepare(
            "SELECT playlist_id, ranked, team_size, result, team_score, opponent_score, metrics_partial
             FROM match_record ORDER BY id",
        )
        .unwrap();
    let outcomes: Vec<(i64, bool, i64, String, i64, i64, bool)> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(
        outcomes,
        [
            (10, true, 1, "win".to_string(), 8, 2, false),
            (11, true, 2, "loss".to_string(), 2, 3, false),
        ]
    );

    let mut stmt = p
        .recorder()
        .conn()
        .prepare(
            "SELECT goals, shots, saves, assists, score, touches, demos FROM match_record ORDER BY id",
        )
        .unwrap();
    let mine: Vec<Row7> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(mine, [(8, 10, 0, 0, 1206, 69, 0), (1, 2, 1, 1, 404, 31, 1)]);
}

#[test]
fn ranked_matches_get_plausible_play_stats() {
    let p = run();
    let mut stmt = p
        .recorder()
        .conn()
        .prepare(
            "SELECT play_seconds, aerial_touches, touches_def + touches_mid + touches_att, touches,
                    big_pads, pct_airborne, pct_possession
             FROM match_record ORDER BY id",
        )
        .unwrap();
    type Row = (f64, i64, i64, i64, i64, f64, f64);
    let rows: Vec<Row> = stmt
        .query_map([], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
                r.get(6)?,
            ))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(rows.len(), 2);
    for (play, aerial, located, touches, pads, airborne, possession) in rows {
        eprintln!(
            "play {play:.0}s aerial {aerial} located {located}/{touches} pads {pads} \
             air {airborne:.1}% possession {possession:.1}%"
        );
        assert!(play > 60.0, "play time {play}");
        assert!(
            located > 0 && located <= touches + 5,
            "{located} of {touches} touches"
        );
        assert!(aerial < located, "{aerial} aerial of {located}");
        assert!(pads as f64 <= play / 4.0, "{pads} big pads in {play} s");
        assert!((1.0..60.0).contains(&airborne), "airborne {airborne}%");
        assert!(
            (5.0..95.0).contains(&possession),
            "possession {possession}%"
        );
    }
}
