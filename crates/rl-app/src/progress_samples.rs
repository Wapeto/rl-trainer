//! Progress fixtures for the UI tests: six weeks of made-up play run through the real progress
//! reads, so the fixtures follow the Rust rules.

use rl_core::store::progress::{
    progress_list, progress_mode, progress_overview, progress_pack, ProgressRange,
};
use serde_json::Value;

/// "Now" for the progress fixtures (the UI tests use the same value).
pub const PROGRESS_NOW: i64 = 1_791_500_000_000;
const DAY: i64 = 86_400_000;

/// Six sessions a week apart. Uncomfortable Saves (pack 1) goes from 12 to 27 makes of 40; Wall
/// Clears (pack 3) is played every other session; a ranked 2v2 match each time, with the Plan 5
/// stats on the last three; a 1v1 match in sessions 3 and 6.
fn seed(conn: &rusqlite::Connection) {
    for (i, days) in [36_i64, 29, 22, 15, 8, 1].into_iter().enumerate() {
        let id = i as i64 + 1;
        let start = PROGRESS_NOW - days * DAY;
        let end = start + 3_600_000;
        let b = id * 10;
        let makes = 9 + 3 * id;
        let goals = id % 3;
        let result = if id % 3 == 0 { "loss" } else { "win" };
        let score = 150 + 20 * id;
        let saves = 1 + id % 2;
        let board = format!(
            r#"[{{"name":"Me","team_num":0,"score":{score},"goals":{goals},"assists":0,"saves":{saves},"shots":3,"demos":0}},{{"name":"Mate","team_num":0,"score":260,"goals":1,"assists":1,"saves":2,"shots":2,"demos":1}}]"#
        );
        let new = if id >= 4 {
            format!(
                "300, {hit}, 4, {aerial}, 6, 5, 3, 70, 8, {air}, {speed}, 18, 9, {boost}, 8, {gspeed}, 1, {poss}",
                hit = 55 + id,
                aerial = id - 2,
                air = 22 - id,
                speed = 60 + id,
                boost = 3000 + 100 * id,
                gspeed = if goals > 0 { (90 + id).to_string() } else { "NULL".into() },
                poss = 48 + id,
            )
        } else {
            "NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL".into()
        };
        conn.execute_batch(&format!(
            "INSERT INTO session (id, started_at, ended_at, reviewed_at)
                 VALUES ({id}, {start}, {end}, {end});
             INSERT INTO block (id, session_id, kind, playlist_id, pack_id, started_at, ended_at)
                 VALUES ({b} + 1, {id}, 'training', 21, 1, {start}, {start} + 1500000);
             WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 40)
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome)
                 SELECT {b} + 1, i, {start}, 4000, CASE WHEN i <= {makes} THEN 'make' ELSE 'miss' END
                 FROM n;
             INSERT INTO block (id, session_id, kind, playlist_id, started_at, ended_at)
                 VALUES ({b} + 2, {id}, 'match', 11, {start} + 1800000, {start} + 2400000);
             INSERT INTO match_record (block_id, match_guid, playlist_id, ranked, team_size,
                 arena, result, team_score, opponent_score, overtime, score, goals, assists,
                 saves, shots, touches, demos, epic_saves, avg_boost, pct_zero_boost,
                 pct_supersonic, metrics_partial, scoreboard_json, play_seconds, hit_avg_speed,
                 hard_hits, aerial_touches, touches_def, touches_mid, touches_att, pct_on_ground,
                 pct_on_wall, pct_airborne, avg_speed, pct_boosting, pct_full_boost, boost_used,
                 big_pads, goal_speed_avg, demos_taken, pct_possession)
                 VALUES ({b} + 2, 'G{b}', 11, 1, 2, 'Stadium_P', '{result}', 2, 1, 0, {score},
                 {goals}, 0, {saves}, 3, 22, 0, 0, 34, 14, 12, 0, '{board}', {new});
             INSERT INTO mmr_entry (team_size, mmr, recorded_at, session_id)
                 VALUES (2, {mmr}, {end}, {id});",
            mmr = 890 + 7 * id - if id == 3 { 15 } else { 0 },
        ))
        .unwrap();
        if id % 2 == 0 {
            conn.execute_batch(&format!(
                "INSERT INTO block (id, session_id, kind, playlist_id, pack_id, started_at, ended_at)
                     VALUES ({b} + 3, {id}, 'training', 21, 3, {start} + 2500000, {start} + 3000000);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 12)
                 INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome)
                     SELECT {b} + 3, i, {start}, 4000, CASE WHEN i <= {m} THEN 'make' ELSE 'miss' END
                     FROM n;",
                m = 4 + id / 2,
            ))
            .unwrap();
        }
    }
}

pub fn progress_samples() -> Vec<(&'static str, Value)> {
    let conn = rl_core::store::open_in_memory().unwrap();
    seed(&conn);
    let range = ProgressRange::Days90;
    let v = |x: serde_json::Result<Value>| x.unwrap();
    vec![
        (
            "progress-list",
            v(serde_json::to_value(
                progress_list(&conn, range, PROGRESS_NOW).unwrap(),
            )),
        ),
        (
            "progress-overview",
            v(serde_json::to_value(
                progress_overview(&conn, range, PROGRESS_NOW).unwrap(),
            )),
        ),
        (
            "progress-mode",
            v(serde_json::to_value(
                progress_mode(&conn, 2, range, PROGRESS_NOW).unwrap(),
            )),
        ),
        (
            "progress-pack",
            v(serde_json::to_value(
                progress_pack(&conn, 1, range, PROGRESS_NOW).unwrap(),
            )),
        ),
    ]
}
