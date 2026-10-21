//! Replay files and the coach's replay stats (spec §4.3), and which recorded matches still need
//! their replay saved (spec §4.4).

use std::collections::{HashMap, HashSet};

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The game's match history keeps the last 20 online matches…
pub const HISTORY_SIZE: i64 = 20;
/// …from the past 30 days.
pub const HISTORY_DAYS: i64 = 30;
/// From this place on, a match is about to drop out of history.
pub const SOON_GONE_PLACE: i64 = 17;
/// "Low boost" for touches and 50/50s, on the 0–100 scale.
pub const LOW_BOOST: f64 = 20.0;
const DAY_MS: i64 = 86_400_000;

/// My stats in one match, from its replay. Percentages are 0–100; boost is 0–100.
#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
pub struct ReplayStats {
    pub tracked_seconds: f64,
    pub pct_behind_ball: f64,
    pub pct_in_front_of_ball: f64,
    pub pct_defensive_third: f64,
    pub pct_neutral_third: f64,
    pub pct_offensive_third: f64,
    /// Team modes only.
    pub secs_first_man: Option<f64>,
    pub secs_second_man: Option<f64>,
    pub secs_third_man: Option<f64>,
    pub became_first_man: Option<i64>,
    pub big_pads: i64,
    pub small_pads: i64,
    pub pads_stolen: i64,
    pub touches: i64,
    pub touches_low_boost: i64,
    pub touches_zero_boost: i64,
    pub fifty_wins: i64,
    pub fifty_losses: i64,
    pub fifty_low_boost: i64,
    pub kickoff_wins: i64,
    pub kickoff_losses: i64,
    pub whiffs: i64,
}

/// One player's stats in a match, from its replay (replay stats page spec §3). Every field is
/// optional: a stat added later reads as missing in older rows. Percentages are 0–100 of the
/// player's tracked time; boost is on the 0–100 scale.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(default)]
pub struct PlayerReplayStats {
    /// Seconds the replay tracked this player (positioning); per-5-minute values divide by it.
    pub tracked_seconds: Option<f64>,
    pub score: Option<f64>,
    pub goals: Option<f64>,
    pub assists: Option<f64>,
    pub saves: Option<f64>,
    pub shots: Option<f64>,
    pub pct_behind_ball: Option<f64>,
    pub pct_in_front_of_ball: Option<f64>,
    pub pct_defensive_third: Option<f64>,
    pub pct_neutral_third: Option<f64>,
    pub pct_offensive_third: Option<f64>,
    pub pct_most_back: Option<f64>,
    pub pct_shadow_defense: Option<f64>,
    pub avg_distance_to_ball_m: Option<f64>,
    pub secs_first_man: Option<f64>,
    pub secs_second_man: Option<f64>,
    pub secs_third_man: Option<f64>,
    pub became_first_man: Option<f64>,
    pub boost_collected: Option<f64>,
    pub boost_used: Option<f64>,
    pub boost_stolen: Option<f64>,
    pub big_pads: Option<f64>,
    pub small_pads: Option<f64>,
    pub pct_zero_boost: Option<f64>,
    pub pct_full_boost: Option<f64>,
    pub boost_used_supersonic: Option<f64>,
    pub touches_low_boost: Option<f64>,
    pub touches: Option<f64>,
    pub hard_hits: Option<f64>,
    pub aerial_touches: Option<f64>,
    pub control_touches: Option<f64>,
    pub first_touches: Option<f64>,
    pub fifty_wins: Option<f64>,
    pub fifty_losses: Option<f64>,
    pub fifty_neutral: Option<f64>,
    pub kickoff_wins: Option<f64>,
    pub kickoff_losses: Option<f64>,
    pub beaten_to_ball: Option<f64>,
    pub whiffs: Option<f64>,
    pub flicks: Option<f64>,
    pub wavedashes: Option<f64>,
    pub half_flips: Option<f64>,
    pub speed_flips: Option<f64>,
    pub avg_speed_kmh: Option<f64>,
    pub pct_supersonic: Option<f64>,
    pub pct_on_ground: Option<f64>,
    pub pct_high_air: Option<f64>,
    pub demos_inflicted: Option<f64>,
    pub demos_taken: Option<f64>,
    pub bumps_inflicted: Option<f64>,
    pub bumps_taken: Option<f64>,
    pub passes_completed: Option<f64>,
    pub passes_received: Option<f64>,
}

/// A player of a replayed match, in the replay's order (team 0 first). No account id is kept.
#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReplayPlayer {
    pub slot: i64,
    pub team: i64,
    pub name: String,
    pub is_me: bool,
    pub is_bot: bool,
    pub stats: PlayerReplayStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayFileStatus {
    Imported,
    Unmatched,
    NotMine,
    Error,
}

impl ReplayFileStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Imported => "imported",
            Self::Unmatched => "unmatched",
            Self::NotMine => "not_mine",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ReplayFileRecord {
    pub path: String,
    pub modified_at: i64,
    pub size: i64,
    pub match_guid: Option<String>,
    pub status: ReplayFileStatus,
    pub error: Option<String>,
}

/// Files already looked at, by path: (time, size). An `unmatched` file whose match has been
/// recorded since is left out, so it is read again: a replay saved on the post-match scoreboard
/// is written before the match is recorded (when the player leaves the match).
pub fn seen_files(conn: &Connection) -> rusqlite::Result<HashMap<String, (i64, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT f.path, f.modified_at, f.size FROM replay_file f
         WHERE NOT (f.status = 'unmatched'
                    AND EXISTS (SELECT 1 FROM match_record m WHERE m.match_guid = f.match_guid))",
    )?;
    let rows = stmt.query_map([], |r| Ok((r.get(0)?, (r.get(1)?, r.get(2)?))))?;
    rows.collect()
}

/// Files whose bytes could not be read at all (no match id came out: offline OneDrive file,
/// locked file). Spec §9: they are tried again once at each start.
pub fn read_failures(conn: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut stmt =
        conn.prepare("SELECT path FROM replay_file WHERE status = 'error' AND match_guid IS NULL")?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect()
}

pub fn record_file(conn: &Connection, rec: &ReplayFileRecord, now_ms: i64) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO replay_file (path, modified_at, size, match_guid, status, error, imported_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(path) DO UPDATE SET modified_at = excluded.modified_at,
             size = excluded.size, match_guid = excluded.match_guid, status = excluded.status,
             error = excluded.error, imported_at = excluded.imported_at",
        params![
            rec.path,
            rec.modified_at,
            rec.size,
            rec.match_guid,
            rec.status.as_str(),
            rec.error,
            now_ms
        ],
    )?;
    Ok(())
}

pub fn match_id_by_guid(conn: &Connection, guid: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM match_record WHERE match_guid = ?1",
        params![guid],
        |r| r.get(0),
    )
    .optional()
}

/// Saves my stats of a match. Reading the replay again (the backfill) keeps its first
/// `imported_at`: the review's "replay added" note reads it.
pub fn save_stats(
    conn: &Connection,
    match_id: i64,
    s: &ReplayStats,
    parser_version: &str,
    now_ms: i64,
) -> rusqlite::Result<()> {
    conn.execute(
        "INSERT INTO replay_stats (match_id, imported_at, parser_version,
             tracked_seconds, pct_behind_ball, pct_in_front_of_ball, pct_defensive_third,
             pct_neutral_third, pct_offensive_third, secs_first_man, secs_second_man,
             secs_third_man, became_first_man, big_pads, small_pads, pads_stolen, touches,
             touches_low_boost, touches_zero_boost, fifty_wins, fifty_losses, fifty_low_boost,
             kickoff_wins, kickoff_losses, whiffs)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
             ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25)
         ON CONFLICT(match_id) DO UPDATE SET
             parser_version = excluded.parser_version, tracked_seconds = excluded.tracked_seconds,
             pct_behind_ball = excluded.pct_behind_ball,
             pct_in_front_of_ball = excluded.pct_in_front_of_ball,
             pct_defensive_third = excluded.pct_defensive_third,
             pct_neutral_third = excluded.pct_neutral_third,
             pct_offensive_third = excluded.pct_offensive_third,
             secs_first_man = excluded.secs_first_man, secs_second_man = excluded.secs_second_man,
             secs_third_man = excluded.secs_third_man,
             became_first_man = excluded.became_first_man, big_pads = excluded.big_pads,
             small_pads = excluded.small_pads, pads_stolen = excluded.pads_stolen,
             touches = excluded.touches, touches_low_boost = excluded.touches_low_boost,
             touches_zero_boost = excluded.touches_zero_boost, fifty_wins = excluded.fifty_wins,
             fifty_losses = excluded.fifty_losses, fifty_low_boost = excluded.fifty_low_boost,
             kickoff_wins = excluded.kickoff_wins, kickoff_losses = excluded.kickoff_losses,
             whiffs = excluded.whiffs",
        params![
            match_id,
            now_ms,
            parser_version,
            s.tracked_seconds,
            s.pct_behind_ball,
            s.pct_in_front_of_ball,
            s.pct_defensive_third,
            s.pct_neutral_third,
            s.pct_offensive_third,
            s.secs_first_man,
            s.secs_second_man,
            s.secs_third_man,
            s.became_first_man,
            s.big_pads,
            s.small_pads,
            s.pads_stolen,
            s.touches,
            s.touches_low_boost,
            s.touches_zero_boost,
            s.fifty_wins,
            s.fifty_losses,
            s.fifty_low_boost,
            s.kickoff_wins,
            s.kickoff_losses,
            s.whiffs
        ],
    )?;
    Ok(())
}

pub(crate) const STATS_COLUMNS: &str = "tracked_seconds, pct_behind_ball,
    pct_in_front_of_ball, pct_defensive_third, pct_neutral_third, pct_offensive_third,
    secs_first_man, secs_second_man, secs_third_man, became_first_man, big_pads, small_pads,
    pads_stolen, touches, touches_low_boost, touches_zero_boost, fifty_wins, fifty_losses,
    fifty_low_boost, kickoff_wins, kickoff_losses, whiffs";

/// Reads `STATS_COLUMNS` starting at column `at`.
pub(crate) fn stats_row(r: &Row, at: usize) -> rusqlite::Result<ReplayStats> {
    Ok(ReplayStats {
        tracked_seconds: r.get(at)?,
        pct_behind_ball: r.get(at + 1)?,
        pct_in_front_of_ball: r.get(at + 2)?,
        pct_defensive_third: r.get(at + 3)?,
        pct_neutral_third: r.get(at + 4)?,
        pct_offensive_third: r.get(at + 5)?,
        secs_first_man: r.get(at + 6)?,
        secs_second_man: r.get(at + 7)?,
        secs_third_man: r.get(at + 8)?,
        became_first_man: r.get(at + 9)?,
        big_pads: r.get(at + 10)?,
        small_pads: r.get(at + 11)?,
        pads_stolen: r.get(at + 12)?,
        touches: r.get(at + 13)?,
        touches_low_boost: r.get(at + 14)?,
        touches_zero_boost: r.get(at + 15)?,
        fifty_wins: r.get(at + 16)?,
        fifty_losses: r.get(at + 17)?,
        fifty_low_boost: r.get(at + 18)?,
        kickoff_wins: r.get(at + 19)?,
        kickoff_losses: r.get(at + 20)?,
        whiffs: r.get(at + 21)?,
    })
}

pub fn stats_for_match(conn: &Connection, match_id: i64) -> rusqlite::Result<Option<ReplayStats>> {
    conn.query_row(
        &format!("SELECT {STATS_COLUMNS} FROM replay_stats WHERE match_id = ?1"),
        params![match_id],
        |r| stats_row(r, 0),
    )
    .optional()
}

/// Replaces a match's player rows.
pub fn save_players(
    conn: &Connection,
    match_id: i64,
    players: &[ReplayPlayer],
) -> rusqlite::Result<()> {
    conn.execute(
        "DELETE FROM replay_player WHERE match_id = ?1",
        params![match_id],
    )?;
    let mut stmt = conn.prepare(
        "INSERT INTO replay_player (match_id, slot, team, name, is_me, is_bot, stats)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;
    for p in players {
        let stats = serde_json::to_string(&p.stats)
            .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
        stmt.execute(params![
            match_id, p.slot, p.team, p.name, p.is_me, p.is_bot, stats
        ])?;
    }
    Ok(())
}

pub fn players_for_match(conn: &Connection, match_id: i64) -> rusqlite::Result<Vec<ReplayPlayer>> {
    let mut stmt = conn.prepare(
        "SELECT slot, team, name, is_me, is_bot, stats FROM replay_player
         WHERE match_id = ?1 ORDER BY slot",
    )?;
    let rows = stmt.query_map(params![match_id], |r| {
        let stats: String = r.get(5)?;
        Ok(ReplayPlayer {
            slot: r.get(0)?,
            team: r.get(1)?,
            name: r.get(2)?,
            is_me: r.get(3)?,
            is_bot: r.get(4)?,
            stats: serde_json::from_str(&stats).map_err(|e| {
                rusqlite::Error::FromSqlConversionFailure(
                    5,
                    rusqlite::types::Type::Text,
                    Box::new(e),
                )
            })?,
        })
    })?;
    rows.collect()
}

/// Matches in the winners' benchmark pool (winners' benchmark spec §2.1).
pub const POOL_MATCHES: i64 = 20;

/// A player of the benchmark pool, on the winning team or not; `is_me` marks my row.
#[derive(Debug, Clone, PartialEq)]
pub struct PoolRow {
    pub won: bool,
    pub is_me: bool,
    pub stats: PlayerReplayStats,
}

/// Every non-bot player of my last `POOL_MATCHES` ranked replays of the same team size before
/// `match_id` with a known result, and how many matches that is. My row gives my team, so the
/// winning team; a replay without it is left out.
pub fn benchmark_pool(conn: &Connection, match_id: i64) -> rusqlite::Result<(usize, Vec<PoolRow>)> {
    let found = conn
        .query_row(
            "SELECT t.team_size, tb.started_at FROM match_record t
             JOIN block tb ON tb.id = t.block_id WHERE t.id = ?1",
            params![match_id],
            |r| Ok((r.get::<_, Option<i64>>(0)?, r.get::<_, i64>(1)?)),
        )
        .optional()?;
    match found {
        Some((Some(team_size), started)) => pool(conn, team_size, Some(started)),
        _ => Ok((0, Vec::new())),
    }
}

/// The same pool up to now: my last `POOL_MATCHES` ranked replays of `team_size` (progress radar spec §3).
pub fn recent_pool(conn: &Connection, team_size: i64) -> rusqlite::Result<(usize, Vec<PoolRow>)> {
    pool(conn, team_size, None)
}

fn pool(
    conn: &Connection,
    team_size: i64,
    before: Option<i64>,
) -> rusqlite::Result<(usize, Vec<PoolRow>)> {
    let mut stmt = conn.prepare(
        "WITH pool AS (
             SELECT m.id, m.result, me.team AS my_team FROM match_record m
             JOIN block b ON b.id = m.block_id
             JOIN replay_player me ON me.match_id = m.id AND me.is_me = 1
             WHERE m.ranked = 1 AND m.team_size = ?1 AND m.result IN ('win', 'loss')
               AND (?2 IS NULL OR b.started_at < ?2)
             ORDER BY b.started_at DESC LIMIT ?3)
         SELECT pool.id, (p.team = pool.my_team) = (pool.result = 'win'), p.is_me, p.stats
         FROM pool JOIN replay_player p ON p.match_id = pool.id
         WHERE p.is_bot = 0",
    )?;
    let rows = stmt.query_map(params![team_size, before, POOL_MATCHES], |r| {
        Ok((
            r.get::<_, i64>(0)?,
            r.get::<_, bool>(1)?,
            r.get::<_, bool>(2)?,
            r.get::<_, String>(3)?,
        ))
    })?;
    let mut ids = HashSet::new();
    let mut pool = Vec::new();
    for row in rows {
        let (id, won, is_me, stats) = row?;
        let stats = serde_json::from_str(&stats).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
        })?;
        ids.insert(id);
        pool.push(PoolRow { won, is_me, stats });
    }
    Ok((ids.len(), pool))
}

pub fn match_team_size(conn: &Connection, match_id: i64) -> rusqlite::Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT team_size FROM match_record WHERE id = ?1",
            params![match_id],
            |r| r.get(0),
        )
        .optional()?
        .flatten())
}

/// Imported replay files whose match has no player rows yet, or whose user row lacks
/// `tracked_seconds` (read before it existed): the poller reads them once more at start.
pub fn missing_players(conn: &Connection) -> rusqlite::Result<HashSet<String>> {
    let mut stmt = conn.prepare(
        "SELECT f.path FROM replay_file f JOIN match_record m ON m.match_guid = f.match_guid
         WHERE f.status = 'imported'
           AND NOT EXISTS (SELECT 1 FROM replay_player p WHERE p.match_id = m.id
                           AND p.is_me = 1
                           AND json_extract(p.stats, '$.tracked_seconds') IS NOT NULL)",
    )?;
    let rows = stmt.query_map([], |r| r.get(0))?;
    rows.collect()
}

/// When the first recorded match started: no replay saved before it can match one.
pub fn earliest_match_at(conn: &Connection) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT MIN(b.started_at) FROM match_record m JOIN block b ON b.id = m.block_id",
        [],
        |r| r.get(0),
    )
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReplayToSave {
    pub match_id: i64,
    pub session_id: i64,
    pub started_at: i64,
    pub ranked: bool,
    pub team_size: Option<i64>,
    pub result: String,
    pub team_score: i64,
    pub opponent_score: i64,
    /// About where it sits in the game's match history (1 = newest).
    pub place: i64,
}

/// SQL for a match's estimated place in history: 1 + the matches that started after it, those
/// deleted from the log included (the game still holds them).
pub(crate) const HISTORY_PLACE: &str = "(SELECT COUNT(*) FROM match_record hm
    JOIN block hb ON hb.id = hm.block_id WHERE hb.started_at > b.started_at)
    + (SELECT COUNT(*) FROM deleted_match dm WHERE dm.started_at > b.started_at) + 1";

/// SQL keeping the start of every match in the blocks `WHERE` picks, before they are deleted.
pub(crate) fn remember_deleted_matches(where_blocks: &str) -> String {
    format!(
        "INSERT INTO deleted_match (started_at, match_guid)
         SELECT b.started_at, m.match_guid FROM match_record m JOIN block b ON b.id = m.block_id
         WHERE {where_blocks}"
    )
}

/// Recorded matches with no replay that are still in the match history, oldest first.
pub fn to_save(conn: &Connection, now_ms: i64) -> rusqlite::Result<Vec<ReplayToSave>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT m.id, b.session_id, b.started_at, m.ranked, m.team_size, m.result, m.team_score,
                m.opponent_score, {HISTORY_PLACE} AS place
         FROM match_record m JOIN block b ON b.id = m.block_id
         WHERE NOT EXISTS (SELECT 1 FROM replay_stats r WHERE r.match_id = m.id)
           AND b.started_at >= ?1
         ORDER BY b.started_at, m.id"
    ))?;
    let rows = stmt.query_map(params![now_ms - HISTORY_DAYS * DAY_MS], |r| {
        Ok(ReplayToSave {
            match_id: r.get(0)?,
            session_id: r.get(1)?,
            started_at: r.get(2)?,
            ranked: r.get(3)?,
            team_size: r.get(4)?,
            result: r.get(5)?,
            team_score: r.get(6)?,
            opponent_score: r.get(7)?,
            place: r.get(8)?,
        })
    })?;
    let all: Vec<ReplayToSave> = rows.collect::<Result<_, _>>()?;
    Ok(all
        .into_iter()
        .filter(|m| m.place <= HISTORY_SIZE)
        .collect())
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, TS)]
pub struct ReplayCounts {
    pub imported: i64,
    pub unmatched: i64,
    pub not_mine: i64,
    pub errors: i64,
    pub last_error: Option<String>,
    /// The last 3 replays of recorded matches all failed: the format may have changed.
    pub format_warning: bool,
}

pub fn counts(conn: &Connection) -> rusqlite::Result<ReplayCounts> {
    let count = |status: ReplayFileStatus| -> rusqlite::Result<i64> {
        conn.query_row(
            "SELECT COUNT(*) FROM replay_file WHERE status = ?1",
            params![status.as_str()],
            |r| r.get(0),
        )
    };
    let last_error: Option<String> = conn
        .query_row(
            "SELECT error FROM replay_file WHERE status = 'error'
             ORDER BY imported_at DESC, id DESC LIMIT 1",
            [],
            |r| r.get(0),
        )
        .optional()?
        .flatten();
    let mut stmt = conn.prepare(
        "SELECT f.status FROM replay_file f
         WHERE f.match_guid IS NOT NULL
           AND EXISTS (SELECT 1 FROM match_record m WHERE m.match_guid = f.match_guid)
         ORDER BY f.imported_at DESC, f.id DESC LIMIT 3",
    )?;
    let last3: Vec<String> = stmt
        .query_map([], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    Ok(ReplayCounts {
        imported: count(ReplayFileStatus::Imported)?,
        unmatched: count(ReplayFileStatus::Unmatched)?,
        not_mine: count(ReplayFileStatus::NotMine)?,
        errors: count(ReplayFileStatus::Error)?,
        last_error,
        format_warning: last3.len() == 3 && last3.iter().all(|s| s == "error"),
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::store::open_in_memory;

    const DAY: i64 = 86_400_000;
    const NOW: i64 = 100 * DAY;

    /// A match `minutes_ago` before NOW in session 1 (ranked unless `ranked` is false).
    pub(crate) fn add_match(conn: &Connection, id: i64, minutes_ago: i64, ranked: bool) {
        let at = NOW - minutes_ago * 60_000;
        conn.execute(
            "INSERT OR IGNORE INTO session (id, started_at) VALUES (1, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO block (id, session_id, kind, started_at, ended_at)
             VALUES (?1, 1, 'match', ?2, ?2 + 300000)",
            params![id, at],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO match_record (id, block_id, match_guid, playlist_id, ranked, team_size,
                 arena, result, team_score, opponent_score, overtime, score, goals, assists,
                 saves, shots, touches, demos, epic_saves, avg_boost, pct_zero_boost,
                 pct_supersonic, metrics_partial, scoreboard_json)
             VALUES (?1, ?1, ?2, 11, ?3, 2, 'Park_P', 'win', 3, 1, 0, 300, 1, 0, 1, 3, 30, 0, 0,
                 40, 15, 10, 0, '[]')",
            params![id, format!("GUID{id}"), ranked],
        )
        .unwrap();
    }

    fn stats() -> ReplayStats {
        ReplayStats {
            tracked_seconds: 300.0,
            pct_behind_ball: 66.0,
            touches: 36,
            touches_low_boost: 17,
            secs_first_man: Some(193.0),
            ..ReplayStats::default()
        }
    }

    #[test]
    fn stats_are_saved_once_per_match_and_read_back() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        assert_eq!(match_id_by_guid(&conn, "GUID1").unwrap(), Some(1));
        assert_eq!(match_id_by_guid(&conn, "nope").unwrap(), None);
        save_stats(&conn, 1, &stats(), "subtr-actor 1.4.0", NOW).unwrap();
        save_stats(&conn, 1, &stats(), "subtr-actor 1.4.0", NOW + 1).unwrap();
        assert_eq!(stats_for_match(&conn, 1).unwrap(), Some(stats()));
        assert_eq!(stats_for_match(&conn, 2).unwrap(), None);
    }

    #[test]
    fn files_are_remembered_by_path_with_their_size_and_time() {
        let conn = open_in_memory().unwrap();
        let rec = |status, size| ReplayFileRecord {
            path: "C:/r/a.replay".into(),
            modified_at: 5,
            size,
            match_guid: None,
            status,
            error: None,
        };
        record_file(&conn, &rec(ReplayFileStatus::Error, 10), NOW).unwrap();
        record_file(&conn, &rec(ReplayFileStatus::Unmatched, 12), NOW).unwrap();
        let seen = seen_files(&conn).unwrap();
        assert_eq!(seen.get("C:/r/a.replay"), Some(&(5, 12)));
        assert_eq!(counts(&conn).unwrap().unmatched, 1);
        assert_eq!(
            counts(&conn).unwrap().errors,
            0,
            "the later state replaces the earlier one"
        );
    }

    #[test]
    fn matches_to_save_are_those_still_in_history() {
        let conn = open_in_memory().unwrap();
        // 22 matches, one every 10 minutes: match 1 is the oldest (place 22).
        for id in 1..=22 {
            add_match(&conn, id, (23 - id) * 10, id != 21);
        }
        save_stats(&conn, 20, &stats(), "v", NOW).unwrap();
        let list = to_save(&conn, NOW).unwrap();
        let ids: Vec<i64> = list.iter().map(|m| m.match_id).collect();
        assert_eq!(ids.first(), Some(&3), "places 21 and 22 have left history");
        assert!(!ids.contains(&20), "it has a replay");
        assert_eq!(list.first().unwrap().place, 20);
        assert_eq!(list.last().unwrap().place, 1);
        assert!(list.iter().any(|m| m.match_id == 21 && !m.ranked));
    }

    fn places(conn: &Connection) -> Vec<(i64, i64)> {
        to_save(conn, NOW)
            .unwrap()
            .iter()
            .map(|m| (m.match_id, m.place))
            .collect()
    }

    #[test]
    fn a_deleted_match_still_takes_its_place_in_history() {
        let conn = open_in_memory().unwrap();
        // 21 matches: match 1 is the oldest (place 21) and has left the game's history.
        for id in 1..=21 {
            add_match(&conn, id, (22 - id) * 10, true);
        }
        conn.execute(
            "UPDATE session SET ended_at = ?1 WHERE id = 1",
            params![NOW],
        )
        .unwrap();
        crate::store::session_edits::delete_activity(&conn, 15).unwrap();
        let list = places(&conn);
        assert!(!list.iter().any(|(id, _)| *id == 1), "still out of history");
        assert_eq!(list.first(), Some(&(2, 20)));
    }

    #[test]
    fn a_deleted_session_keeps_its_matches_places_in_history() {
        let conn = open_in_memory().unwrap();
        for id in 1..=21 {
            add_match(&conn, id, (22 - id) * 10, true);
        }
        conn.execute(
            "INSERT INTO session (id, started_at, ended_at) VALUES (2, ?1, ?1)",
            params![NOW],
        )
        .unwrap();
        conn.execute("UPDATE block SET session_id = 2 WHERE id IN (20, 21)", [])
            .unwrap();
        crate::store::session_edits::delete_session(&conn, 2).unwrap();
        let list = places(&conn);
        assert!(!list.iter().any(|(id, _)| *id == 1), "still out of history");
        assert_eq!(list.first(), Some(&(2, 20)));
    }

    #[test]
    fn matches_older_than_thirty_days_are_out_of_history() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 31 * 24 * 60, true);
        add_match(&conn, 2, 29 * 24 * 60, true);
        let ids: Vec<i64> = to_save(&conn, NOW)
            .unwrap()
            .iter()
            .map(|m| m.match_id)
            .collect();
        assert_eq!(ids, [2]);
    }

    #[test]
    fn the_earliest_match_bounds_which_files_can_match() {
        let conn = open_in_memory().unwrap();
        assert_eq!(earliest_match_at(&conn).unwrap(), None);
        add_match(&conn, 1, 60, true);
        add_match(&conn, 2, 30, true);
        assert_eq!(earliest_match_at(&conn).unwrap(), Some(NOW - 60 * 60_000));
    }

    #[test]
    fn three_failed_matched_replays_in_a_row_raise_the_format_warning() {
        let conn = open_in_memory().unwrap();
        for id in 1..=3 {
            add_match(&conn, id, 60 - id, true);
            record_file(
                &conn,
                &ReplayFileRecord {
                    path: format!("C:/r/{id}.replay"),
                    modified_at: id,
                    size: 1,
                    match_guid: Some(format!("GUID{id}")),
                    status: ReplayFileStatus::Error,
                    error: Some("bad".into()),
                },
                NOW + id,
            )
            .unwrap();
        }
        let c = counts(&conn).unwrap();
        assert!(c.format_warning);
        assert_eq!(c.last_error.as_deref(), Some("bad"));
    }

    #[test]
    fn a_schema_8_database_gains_the_replay_tables_and_keeps_its_matches() {
        let mut conn = Connection::open_in_memory().unwrap();
        crate::store::migrations::migrate_to(&mut conn, 8).unwrap();
        add_match(&conn, 1, 60, true);
        crate::store::migrations::migrate(&mut conn).unwrap();
        save_stats(&conn, 1, &stats(), "v", NOW).unwrap();
        assert_eq!(stats_for_match(&conn, 1).unwrap(), Some(stats()));
        assert_eq!(counts(&conn).unwrap(), ReplayCounts::default());
    }

    #[test]
    fn a_replay_saved_before_its_match_was_recorded_is_read_again() {
        // Saved on the post-match scoreboard: the file can be read before the match row exists.
        let conn = open_in_memory().unwrap();
        let rec = |guid: &str| ReplayFileRecord {
            path: format!("C:/r/{guid}.replay"),
            modified_at: 5,
            size: 10,
            match_guid: Some(guid.into()),
            status: ReplayFileStatus::Unmatched,
            error: None,
        };
        record_file(&conn, &rec("GUID1"), NOW).unwrap();
        record_file(&conn, &rec("OTHER"), NOW).unwrap();
        assert_eq!(seen_files(&conn).unwrap().len(), 2);
        add_match(&conn, 1, 1, true);
        let seen = seen_files(&conn).unwrap();
        assert!(
            !seen.contains_key("C:/r/GUID1.replay"),
            "its match is recorded now: read it again"
        );
        assert!(
            seen.contains_key("C:/r/OTHER.replay"),
            "still no match: not read again"
        );
    }

    #[test]
    fn files_that_could_not_be_read_are_listed_for_a_retry() {
        let conn = open_in_memory().unwrap();
        let rec = |name: &str, guid: Option<&str>| ReplayFileRecord {
            path: format!("C:/r/{name}.replay"),
            modified_at: 5,
            size: 10,
            match_guid: guid.map(str::to_owned),
            status: ReplayFileStatus::Error,
            error: Some("bad".into()),
        };
        record_file(&conn, &rec("offline", None), NOW).unwrap();
        record_file(&conn, &rec("parsed", Some("GUID9")), NOW).unwrap();
        let retry = read_failures(&conn).unwrap();
        assert!(
            retry.contains("C:/r/offline.replay"),
            "the file itself could not be read"
        );
        assert!(
            !retry.contains("C:/r/parsed.replay"),
            "its stats pass failed: same result again"
        );
    }

    fn player(slot: i64, name: &str) -> ReplayPlayer {
        ReplayPlayer {
            slot,
            team: slot % 2,
            name: name.into(),
            is_me: slot == 0,
            is_bot: false,
            stats: PlayerReplayStats {
                goals: Some(1.0),
                avg_speed_kmh: Some(55.5),
                ..PlayerReplayStats::default()
            },
        }
    }

    #[test]
    fn players_round_trip_and_are_replaced() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        save_players(&conn, 1, &[player(1, "B"), player(0, "A")]).unwrap();
        save_players(&conn, 1, &[player(0, "A"), player(1, "B")]).unwrap();
        let got = players_for_match(&conn, 1).unwrap();
        assert_eq!(
            got,
            vec![player(0, "A"), player(1, "B")],
            "slot order, no duplicates"
        );
        assert!(players_for_match(&conn, 2).unwrap().is_empty());
    }

    #[test]
    fn a_stats_row_missing_a_field_reads_it_as_none() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        conn.execute(
            "INSERT INTO replay_player (match_id, slot, team, name, is_me, is_bot, stats)
             VALUES (1, 0, 0, 'A', 1, 0, '{\"goals\": 2}')",
            [],
        )
        .unwrap();
        let got = &players_for_match(&conn, 1).unwrap()[0].stats;
        assert_eq!(got.goals, Some(2.0));
        assert_eq!(got.whiffs, None);
    }

    #[test]
    fn deleting_the_match_deletes_its_players() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        save_players(&conn, 1, &[player(0, "A")]).unwrap();
        conn.execute("DELETE FROM match_record WHERE id = 1", [])
            .unwrap();
        assert!(players_for_match(&conn, 1).unwrap().is_empty());
    }

    #[test]
    fn missing_players_lists_imported_replays_without_player_rows() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        add_match(&conn, 2, 50, true);
        let rec = |path: &str, guid: Option<&str>, status| ReplayFileRecord {
            path: path.into(),
            modified_at: 1,
            size: 1,
            match_guid: guid.map(Into::into),
            status,
            error: None,
        };
        record_file(
            &conn,
            &rec("a", Some("GUID1"), ReplayFileStatus::Imported),
            NOW,
        )
        .unwrap();
        record_file(
            &conn,
            &rec("b", Some("GUID2"), ReplayFileStatus::Imported),
            NOW,
        )
        .unwrap();
        record_file(&conn, &rec("c", None, ReplayFileStatus::Unmatched), NOW).unwrap();
        let mut timed = player(0, "A");
        timed.stats.tracked_seconds = Some(300.0);
        save_players(&conn, 2, &[timed]).unwrap();
        assert_eq!(
            missing_players(&conn).unwrap(),
            HashSet::from(["a".to_string()])
        );
    }

    #[test]
    fn the_pool_is_every_player_of_my_last_ranked_replays_in_the_mode() {
        let conn = open_in_memory().unwrap();
        for id in 1..=7 {
            add_match(&conn, id, 100 - id * 10, true); // 1 is oldest; all 2v2 wins
        }
        conn.execute("UPDATE match_record SET result = 'loss' WHERE id = 2", [])
            .unwrap();
        conn.execute("UPDATE match_record SET team_size = 1 WHERE id = 3", [])
            .unwrap();
        conn.execute("UPDATE match_record SET ranked = 0 WHERE id = 4", [])
            .unwrap();
        conn.execute(
            "UPDATE match_record SET result = 'unknown' WHERE id = 5",
            [],
        )
        .unwrap();
        let bot = ReplayPlayer {
            is_bot: true,
            ..player(3, "Bot")
        };
        for id in [1, 2, 3, 4, 5, 7] {
            // slot 0 is me on team 0; slots 1 and 3 are team 1
            save_players(
                &conn,
                id,
                &[player(0, "Me"), player(1, "Them"), bot.clone()],
            )
            .unwrap();
        }
        save_players(&conn, 6, &[player(1, "Them")]).unwrap(); // no row of mine

        let (matches, rows) = benchmark_pool(&conn, 7).unwrap();
        assert_eq!(
            matches, 2,
            "only 1 and 2: 3 is 1v1, 4 casual, 5 unknown, 6 not mine, 7 is the match"
        );
        let mut won: Vec<bool> = rows.iter().map(|r| r.won).collect();
        won.sort();
        assert_eq!(
            won,
            [false, false, true, true],
            "1 won: me won; 2 lost: them won; no bot"
        );
        assert_eq!(
            rows.iter().filter(|r| r.is_me).count(),
            2,
            "my row in 1 and 2"
        );
        assert_eq!(
            benchmark_pool(&conn, 99).unwrap(),
            (0, Vec::new()),
            "no such match"
        );
        assert_eq!(
            benchmark_pool(&conn, 1).unwrap().0,
            0,
            "nothing before the first match"
        );
        assert_eq!(match_team_size(&conn, 3).unwrap(), Some(1));
        assert_eq!(match_team_size(&conn, 99).unwrap(), None);
    }

    #[test]
    fn the_pool_keeps_the_last_twenty_matches() {
        let conn = open_in_memory().unwrap();
        for id in 1..=25 {
            add_match(&conn, id, 1000 - id * 10, true);
            save_players(&conn, id, &[player(0, "Me")]).unwrap();
        }
        let (matches, rows) = benchmark_pool(&conn, 25).unwrap();
        assert_eq!((matches, rows.len()), (20, 20));
    }

    #[test]
    fn the_recent_pool_has_no_cut_off_and_flags_my_rows() {
        let conn = open_in_memory().unwrap();
        for id in 1..=25 {
            add_match(&conn, id, 1000 - id * 10, true);
            save_players(&conn, id, &[player(0, "Me"), player(1, "Them")]).unwrap();
        }
        conn.execute("UPDATE match_record SET team_size = 1 WHERE id = 25", [])
            .unwrap();
        let (matches, rows) = recent_pool(&conn, 2).unwrap();
        assert_eq!(matches, 20, "the latest 2v2s, 24 down to 5");
        assert_eq!(rows.iter().filter(|r| r.is_me).count(), 20);
        assert_eq!(rows.iter().filter(|r| !r.is_me).count(), 20);
        assert_eq!(recent_pool(&conn, 1).unwrap().0, 1);
        assert_eq!(recent_pool(&conn, 3).unwrap().0, 0);
    }

    #[test]
    fn missing_players_also_lists_replays_whose_rows_lack_tracked_time() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        let rec = ReplayFileRecord {
            path: "a".into(),
            modified_at: 1,
            size: 1,
            match_guid: Some("GUID1".into()),
            status: ReplayFileStatus::Imported,
            error: None,
        };
        record_file(&conn, &rec, NOW).unwrap();
        save_players(&conn, 1, &[player(0, "A")]).unwrap();
        assert_eq!(
            missing_players(&conn).unwrap(),
            HashSet::from(["a".to_string()])
        );
        let mut timed = player(0, "A");
        timed.stats.tracked_seconds = Some(300.0);
        save_players(&conn, 1, &[timed]).unwrap();
        assert!(missing_players(&conn).unwrap().is_empty());
    }

    #[test]
    fn a_failed_reimport_keeps_the_old_replay_stats() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        let rec = |status| ReplayFileRecord {
            path: "a".into(),
            modified_at: 1,
            size: 1,
            match_guid: Some("GUID1".into()),
            status,
            error: None,
        };
        record_file(&conn, &rec(ReplayFileStatus::Imported), NOW).unwrap();
        save_stats(&conn, 1, &stats(), "v", NOW).unwrap();
        record_file(&conn, &rec(ReplayFileStatus::Error), NOW).unwrap();
        assert_eq!(stats_for_match(&conn, 1).unwrap(), Some(stats()));
        assert!(
            missing_players(&conn).unwrap().is_empty(),
            "an error is not retried every start"
        );
        assert!(
            !read_failures(&conn).unwrap().contains("a"),
            "it had a match id: not a read failure"
        );
    }

    #[test]
    fn reading_a_replay_again_keeps_when_it_was_first_imported() {
        let conn = open_in_memory().unwrap();
        add_match(&conn, 1, 60, true);
        save_stats(&conn, 1, &stats(), "v", NOW).unwrap();
        let mut later = stats();
        later.whiffs += 1;
        save_stats(&conn, 1, &later, "v", NOW + 1_000).unwrap();
        let at: i64 = conn
            .query_row(
                "SELECT imported_at FROM replay_stats WHERE match_id = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            at, NOW,
            "a backfill must not look like a newly added replay"
        );
        assert_eq!(stats_for_match(&conn, 1).unwrap(), Some(later));
    }
}
