//! Small databases for the progress tests. Times are relative to `NOW`.

use rusqlite::{params, Connection};

use super::DAY_MS;
use crate::store::open_in_memory;

pub const NOW: i64 = 1_791_500_000_000;

pub fn db() -> Connection {
    open_in_memory().unwrap()
}

fn session_start(conn: &Connection, session: i64) -> i64 {
    conn.query_row(
        "SELECT started_at FROM session WHERE id = ?1",
        [session],
        |r| r.get(0),
    )
    .unwrap()
}

/// A finished, one-hour session `days_ago` days before `NOW`.
pub fn session(conn: &Connection, id: i64, days_ago: i64) {
    let start = NOW - days_ago * DAY_MS;
    conn.execute(
        "INSERT INTO session (id, started_at, ended_at) VALUES (?1, ?2, ?3)",
        params![id, start, start + 3_600_000],
    )
    .unwrap();
}

/// A training activity starting `block % 10` × 10 minutes into the session, with `makes` of
/// `attempts` counted attempts and one skipped make that never counts.
pub fn training(
    conn: &Connection,
    block: i64,
    session: i64,
    pack: Option<i64>,
    attempts: i64,
    makes: i64,
    minutes: i64,
) {
    let start = session_start(conn, session) + (block % 10) * 600_000;
    conn.execute(
        "INSERT INTO block (id, session_id, kind, playlist_id, pack_id, started_at, ended_at)
         VALUES (?1, ?2, 'training', 21, ?3, ?4, ?5)",
        params![block, session, pack, start, start + minutes * 60_000],
    )
    .unwrap();
    for seq in 1..=attempts {
        let outcome = if seq <= makes { "make" } else { "miss" };
        conn.execute(
            "INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome)
             VALUES (?1, ?2, ?3, 3000, ?4)",
            params![block, seq, start, outcome],
        )
        .unwrap();
    }
    conn.execute(
        "INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome, skipped)
         VALUES (?1, ?2, ?3, 3000, 'make', 1)",
        params![block, attempts + 1, start],
    )
    .unwrap();
}

/// A 5-minute ranked match starting `block % 10` × 10 minutes into the session. My scoreboard
/// line: score 200, `goals`, 0 assists, 1 save, `shots`; my mate: 300, 1, 0, 3, 2. With `play`
/// the Plan 5 stats are set (airborne 20%, 2 aerials, 8 big pads, 10 boost a second); without,
/// they stay NULL like a match recorded before Plan 5.
#[allow(clippy::too_many_arguments)]
pub fn ranked(
    conn: &Connection,
    block: i64,
    session: i64,
    team_size: i64,
    result: &str,
    goals: i64,
    shots: i64,
    play: Option<f64>,
) {
    let start = session_start(conn, session) + (block % 10) * 600_000;
    conn.execute(
        "INSERT INTO block (id, session_id, kind, playlist_id, started_at, ended_at)
         VALUES (?1, ?2, 'match', 11, ?3, ?4)",
        params![block, session, start, start + 300_000],
    )
    .unwrap();
    let board = format!(
        r#"[{{"name":"Me","team_num":0,"score":200,"goals":{goals},"assists":0,"saves":1,"shots":{shots},"demos":0}},{{"name":"Mate","team_num":0,"score":300,"goals":1,"assists":0,"saves":3,"shots":2,"demos":0}}]"#
    );
    conn.execute(
        "INSERT INTO match_record (block_id, match_guid, playlist_id, ranked, team_size, arena,
             result, team_score, opponent_score, overtime, score, goals, assists, saves, shots,
             touches, demos, epic_saves, avg_boost, pct_zero_boost, pct_supersonic,
             metrics_partial, scoreboard_json, play_seconds, pct_airborne, aerial_touches,
             big_pads, boost_used)
         VALUES (?1, ?2, 11, 1, ?3, 'Stadium_P', ?4, 2, 1, 0, 200, ?5, 0, 1, ?6, 20, 0, 0, 35.0,
             12.0, 14.0, 0, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            block,
            format!("G{block}"),
            team_size,
            result,
            goals,
            shots,
            board,
            play,
            play.map(|_| 20.0),
            play.map(|_| 2),
            play.map(|_| 8),
            play.map(|s| s * 10.0)
        ],
    )
    .unwrap();
}

pub fn mmr(conn: &Connection, team_size: i64, value: i64, days_ago: i64, session: Option<i64>) {
    conn.execute(
        "INSERT INTO mmr_entry (team_size, mmr, recorded_at, session_id) VALUES (?1, ?2, ?3, ?4)",
        params![
            team_size,
            value,
            NOW - days_ago * DAY_MS + 3_000_000,
            session
        ],
    )
    .unwrap();
}
