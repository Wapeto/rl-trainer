//! Session read models for the desktop: list cards, the review count and one session in full.

use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::playlist::Activity;
use crate::store::attempt_sql::{counted, made};
use crate::store::packs::code_key;
use crate::store::replays::HISTORY_PLACE;

/// Free play shorter than this is free play while queueing: hidden on the desktop (spec §7.1).
pub const SHORT_FREEPLAY_MS: i64 = 60_000;

fn is_short_freeplay(a: &ActivityRow) -> bool {
    a.kind == Activity::Freeplay
        && a.ended_at
            .is_some_and(|end| end - a.started_at < SHORT_FREEPLAY_MS)
}

/// Training without a pack is measured against the pack table's default target.
pub const DEFAULT_TARGET_RATE: f64 = 0.65;
pub const MAX_PAGE: i64 = 200;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SessionCard {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    /// Distinct packs trained; training without a pack counts as one.
    pub packs: i64,
    pub ranked_matches: i64,
    pub casual_matches: i64,
    pub freeplay_activities: i64,
    pub needs_review: bool,
}

/// A finished session with a finished activity, never reviewed or played on after its review.
const NEEDS_REVIEW: &str = "(s.ended_at IS NOT NULL
      AND EXISTS (SELECT 1 FROM block b WHERE b.session_id = s.id AND b.ended_at IS NOT NULL)
      AND (s.reviewed_at IS NULL
           OR EXISTS (SELECT 1 FROM block b
                      WHERE b.session_id = s.id AND b.ended_at > s.reviewed_at)))";

/// `%text%` with LIKE's own wildcards taken literally (`ESCAPE '\'`).
fn like_pattern(text: &str) -> String {
    let mut out = String::from("%");
    for c in text.chars() {
        if matches!(c, '%' | '_' | '\\') {
            out.push('\\');
        }
        out.push(c);
    }
    out.push('%');
    out
}

pub fn session_cards(
    conn: &Connection,
    search: &str,
    offset: i64,
    limit: i64,
) -> rusqlite::Result<Vec<SessionCard>> {
    let query = search.trim();
    let key = code_key(query);
    let sql = format!(
        "SELECT {}
         FROM session s
         WHERE ?1 = '' OR s.notes LIKE ?2 ESCAPE '\\'
            OR EXISTS (SELECT 1 FROM block b JOIN pack p ON p.id = b.pack_id
                       WHERE b.session_id = s.id
                         AND (p.name LIKE ?2 ESCAPE '\\'
                              OR (?3 != '' AND REPLACE(REPLACE(UPPER(p.code), '-', ''), ' ', '') LIKE ?4)))
         ORDER BY s.started_at DESC, s.id DESC
         LIMIT ?5 OFFSET ?6",
        card_columns()
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(
        params![
            query,
            like_pattern(query),
            key,
            format!("%{key}%"),
            limit.clamp(1, MAX_PAGE),
            offset.max(0)
        ],
        card_row,
    )?;
    rows.collect()
}

/// The card columns of session `s`, read by `card_row`.
fn card_columns() -> String {
    format!(
        "s.id, s.started_at, s.ended_at,
            (SELECT COUNT(DISTINCT COALESCE(b.pack_id, -1)) FROM block b
               WHERE b.session_id = s.id AND b.kind = 'training'),
            (SELECT COUNT(*) FROM block b JOIN match_record m ON m.block_id = b.id
               WHERE b.session_id = s.id AND m.ranked = 1),
            (SELECT COUNT(*) FROM block b JOIN match_record m ON m.block_id = b.id
               WHERE b.session_id = s.id AND m.ranked = 0),
            (SELECT COUNT(*) FROM block b WHERE b.session_id = s.id AND b.kind = 'freeplay'
               AND (b.ended_at IS NULL OR b.ended_at - b.started_at >= {SHORT_FREEPLAY_MS})),
            {NEEDS_REVIEW}"
    )
}

fn card_row(r: &Row) -> rusqlite::Result<SessionCard> {
    Ok(SessionCard {
        id: r.get(0)?,
        started_at: r.get(1)?,
        ended_at: r.get(2)?,
        packs: r.get(3)?,
        ranked_matches: r.get(4)?,
        casual_matches: r.get(5)?,
        freeplay_activities: r.get(6)?,
        needs_review: r.get(7)?,
    })
}

/// Cards of the given sessions, newest first; ids that do not exist are left out.
pub fn session_cards_in(conn: &Connection, ids: &[i64]) -> rusqlite::Result<Vec<SessionCard>> {
    let sql = format!("SELECT {} FROM session s WHERE s.id = ?1", card_columns());
    let mut stmt = conn.prepare(&sql)?;
    let mut cards = Vec::new();
    for id in ids {
        if let Some(card) = stmt.query_row(params![id], card_row).optional()? {
            cards.push(card);
        }
    }
    cards.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(b.id.cmp(&a.id)));
    Ok(cards)
}

pub fn review_count(conn: &Connection) -> rusqlite::Result<i64> {
    conn.query_row(
        &format!("SELECT COUNT(*) FROM session s WHERE {NEEDS_REVIEW}"),
        [],
        |r| r.get(0),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Neighbour {
    Previous,
    Next,
}

/// The session just before or after (by start time, then id).
pub(crate) fn neighbour(
    conn: &Connection,
    started_at: i64,
    id: i64,
    which: Neighbour,
) -> rusqlite::Result<Option<i64>> {
    let sql = match which {
        Neighbour::Previous => {
            "SELECT id FROM session WHERE started_at < ?1 OR (started_at = ?1 AND id < ?2)
             ORDER BY started_at DESC, id DESC LIMIT 1"
        }
        Neighbour::Next => {
            "SELECT id FROM session WHERE started_at > ?1 OR (started_at = ?1 AND id > ?2)
             ORDER BY started_at, id LIMIT 1"
        }
    };
    conn.query_row(sql, params![started_at, id], |r| r.get(0))
        .optional()
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackRef {
    pub id: i64,
    pub name: String,
    pub code: String,
    pub target_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct FreeplaySummary {
    pub touches: i64,
    pub hard_hits: i64,
    pub max_hit_speed: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MatchSummary {
    pub ranked: bool,
    pub team_size: Option<i64>,
    pub arena: String,
    /// `win`, `loss` or `unknown`.
    pub result: String,
    pub team_score: i64,
    pub opponent_score: i64,
    pub overtime: bool,
    pub score: i64,
    pub goals: i64,
    pub assists: i64,
    pub saves: i64,
    pub match_id: i64,
    /// About where the match sits in the game's match history (1 = newest; over 20 = gone).
    pub history_place: i64,
    /// My replay stats, when its replay was imported.
    pub replay: Option<crate::store::replays::ReplayStats>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ActivityRow {
    pub id: i64,
    pub kind: Activity,
    pub playlist_id: Option<i32>,
    pub started_at: i64,
    /// `None` while it is being recorded.
    pub ended_at: Option<i64>,
    pub pack: Option<PackRef>,
    /// Training only; skipped attempts left out.
    pub attempts: i64,
    pub makes: i64,
    pub freeplay: Option<FreeplaySummary>,
    pub match_summary: Option<MatchSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct MmrRow {
    pub id: i64,
    pub team_size: u8,
    pub mmr: i32,
    pub recorded_at: i64,
    /// Against the previous entry for the same team size (any session); `None` for the first.
    pub change: Option<i32>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SessionTotals {
    pub attempts: i64,
    pub makes: i64,
    /// Targets of the trained packs weighted by attempts (no pack: `DEFAULT_TARGET_RATE`).
    pub target_rate: Option<f64>,
    /// Ranked matches only.
    pub wins: i64,
    pub losses: i64,
    pub freeplay_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SessionDetail {
    pub id: i64,
    pub started_at: i64,
    pub ended_at: Option<i64>,
    pub notes: String,
    pub needs_review: bool,
    pub previous_id: Option<i64>,
    pub next_id: Option<i64>,
    pub activities: Vec<ActivityRow>,
    pub mmr: Vec<MmrRow>,
    pub totals: SessionTotals,
}

fn activity_kind(kind: &str) -> Activity {
    match kind {
        "freeplay" => Activity::Freeplay,
        "training" => Activity::Training,
        "match" => Activity::Match,
        _ => Activity::Other,
    }
}

fn activities(conn: &Connection, session_id: i64) -> rusqlite::Result<Vec<ActivityRow>> {
    let sql = format!(
        "SELECT b.id, b.kind, b.playlist_id, b.started_at, b.ended_at,
                p.id, p.name, p.code, p.target_rate,
                (SELECT COUNT(*) FROM training_attempt a WHERE a.block_id = b.id AND {c}),
                (SELECT COUNT(*) FROM training_attempt a
                   WHERE a.block_id = b.id AND {c} AND {m}),
                f.touches, f.hard_hits, f.max_hit_speed,
                m.ranked, m.team_size, m.arena, m.result, m.team_score, m.opponent_score,
                m.overtime, m.score, m.goals, m.assists, m.saves, m.id, {place}
         FROM block b
         LEFT JOIN pack p ON p.id = b.pack_id
         LEFT JOIN freeplay_stats f ON f.block_id = b.id
         LEFT JOIN match_record m ON m.block_id = b.id
         WHERE b.session_id = ?1
         ORDER BY b.started_at, b.id",
        c = counted("a"),
        m = made("a"),
        place = HISTORY_PLACE
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![session_id], |r| {
        let kind: String = r.get(1)?;
        let pack = match r.get::<_, Option<i64>>(5)? {
            Some(id) => Some(PackRef {
                id,
                name: r.get(6)?,
                code: r.get(7)?,
                target_rate: r.get(8)?,
            }),
            None => None,
        };
        let freeplay = match r.get::<_, Option<i64>>(11)? {
            Some(touches) => Some(FreeplaySummary {
                touches,
                hard_hits: r.get(12)?,
                max_hit_speed: r.get(13)?,
            }),
            None => None,
        };
        let match_summary = match r.get::<_, Option<bool>>(14)? {
            Some(ranked) => Some(MatchSummary {
                ranked,
                team_size: r.get(15)?,
                arena: r.get(16)?,
                result: r.get(17)?,
                team_score: r.get(18)?,
                opponent_score: r.get(19)?,
                overtime: r.get(20)?,
                score: r.get(21)?,
                goals: r.get(22)?,
                assists: r.get(23)?,
                saves: r.get(24)?,
                match_id: r.get(25)?,
                history_place: r.get(26)?,
                replay: None,
            }),
            None => None,
        };
        Ok(ActivityRow {
            id: r.get(0)?,
            kind: activity_kind(&kind),
            playlist_id: r.get(2)?,
            started_at: r.get(3)?,
            ended_at: r.get(4)?,
            pack,
            attempts: r.get(9)?,
            makes: r.get(10)?,
            freeplay,
            match_summary,
        })
    })?;
    let mut rows: Vec<ActivityRow> = rows.collect::<Result<_, _>>()?;
    rows.retain(|a| !is_short_freeplay(a));
    for row in &mut rows {
        if let Some(m) = row.match_summary.as_mut() {
            m.replay = crate::store::replays::stats_for_match(conn, m.match_id)?;
        }
    }
    Ok(rows)
}

fn mmr_rows(conn: &Connection, session_id: i64) -> rusqlite::Result<Vec<MmrRow>> {
    let mut stmt = conn.prepare(
        "SELECT e.id, e.team_size, e.mmr, e.recorded_at,
                e.mmr - (SELECT p.mmr FROM mmr_entry p
                         WHERE p.team_size = e.team_size
                           AND (p.recorded_at < e.recorded_at
                                OR (p.recorded_at = e.recorded_at AND p.id < e.id))
                         ORDER BY p.recorded_at DESC, p.id DESC LIMIT 1)
         FROM mmr_entry e WHERE e.session_id = ?1
         ORDER BY e.recorded_at, e.id",
    )?;
    let rows = stmt.query_map(params![session_id], |r| {
        Ok(MmrRow {
            id: r.get(0)?,
            team_size: r.get(1)?,
            mmr: r.get(2)?,
            recorded_at: r.get(3)?,
            change: r.get(4)?,
        })
    })?;
    rows.collect()
}

fn totals(activities: &[ActivityRow]) -> SessionTotals {
    let training: Vec<&ActivityRow> = activities
        .iter()
        .filter(|a| a.kind == Activity::Training)
        .collect();
    let attempts: i64 = training.iter().map(|a| a.attempts).sum();
    let weighted: f64 = training
        .iter()
        .map(|a| {
            a.attempts as f64
                * a.pack
                    .as_ref()
                    .map_or(DEFAULT_TARGET_RATE, |p| p.target_rate)
        })
        .sum();
    let ranked = || {
        activities
            .iter()
            .filter_map(|a| a.match_summary.as_ref())
            .filter(|m| m.ranked)
    };
    SessionTotals {
        attempts,
        makes: training.iter().map(|a| a.makes).sum(),
        target_rate: (attempts > 0).then(|| weighted / attempts as f64),
        wins: ranked().filter(|m| m.result == "win").count() as i64,
        losses: ranked().filter(|m| m.result == "loss").count() as i64,
        freeplay_ms: activities
            .iter()
            .filter(|a| a.kind == Activity::Freeplay)
            .filter_map(|a| a.ended_at.map(|end| end - a.started_at))
            .sum(),
    }
}

pub fn session_detail(conn: &Connection, id: i64) -> rusqlite::Result<Option<SessionDetail>> {
    let head: Option<(i64, Option<i64>, String, bool)> = conn
        .query_row(
            &format!(
                "SELECT s.started_at, s.ended_at, s.notes, {NEEDS_REVIEW} FROM session s WHERE s.id = ?1"
            ),
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    let Some((started_at, ended_at, notes, needs_review)) = head else {
        return Ok(None);
    };
    let activities = activities(conn, id)?;
    Ok(Some(SessionDetail {
        id,
        started_at,
        ended_at,
        notes,
        needs_review,
        previous_id: neighbour(conn, started_at, id, Neighbour::Previous)?,
        next_id: neighbour(conn, started_at, id, Neighbour::Next)?,
        mmr: mmr_rows(conn, id)?,
        totals: totals(&activities),
        activities,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::open_in_memory;

    /// 1: finished, not reviewed (free play, Wall Clears 2/3, unassigned 0/1, two ranked 2v2);
    /// 2: a casual 3v3, reviewed at its end; 3: open, training Aerial Shots - Pass.
    fn detail_db() -> Connection {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at, notes, reviewed_at) VALUES
                (1, -59000, 9000, 'warm-up', NULL), (2, 20000, 30000, '', 30000),
                (3, 40000, NULL, '', NULL);
             UPDATE pack SET target_rate = 0.5 WHERE id = 3;
             INSERT INTO block (id, session_id, kind, playlist_id, started_at, ended_at, pack_id) VALUES
                (10, 1, 'freeplay', 9, -59000, 2000, NULL), (11, 1, 'training', 21, 2000, 4000, 3),
                (12, 1, 'training', 21, 4000, 5000, NULL), (13, 1, 'match', 11, 5000, 7000, NULL),
                (14, 1, 'match', 11, 7000, 9000, NULL), (20, 2, 'match', 6, 20000, 30000, NULL),
                (30, 3, 'training', 21, 40000, NULL, 11);
             INSERT INTO freeplay_stats (block_id, touches, hard_hits, avg_hit_speed, max_hit_speed,
                 goals, crossbar_hits, pct_supersonic, pct_on_wall, pct_airborne)
                 VALUES (10, 64, 9, 70.0, 118.0, 3, 0, 10.0, 5.0, 22.0);
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome, skipped) VALUES
                (11, 1, 2000, 3000, 'make', 0), (11, 2, 2100, 3000, 'make', 0),
                (11, 3, 2200, 3000, 'miss', 0), (11, 4, 2300, 3000, 'make', 1),
                (12, 1, 4000, 3000, 'miss', 0);
             INSERT INTO match_record (block_id, match_guid, playlist_id, ranked, team_size, arena,
                 result, team_score, opponent_score, overtime, score, goals, assists, saves, shots,
                 touches, demos, epic_saves, avg_boost, pct_zero_boost, pct_supersonic,
                 metrics_partial, scoreboard_json) VALUES
                (13, 'g13', 11, 1, 2, 'Stadium_P', 'win', 3, 1, 0, 412, 2, 0, 1, 3, 20, 0, 0, 30.0, 10.0, 15.0, 0, '[]'),
                (14, 'g14', 11, 1, 2, 'EuroStadium_P', 'loss', 1, 2, 1, 288, 1, 0, 2, 2, 18, 0, 0, 30.0, 10.0, 15.0, 0, '[]'),
                (20, 'g20', 6, 0, 3, 'Park_P', 'win', 4, 2, 0, 300, 1, 1, 1, 2, 15, 0, 0, 30.0, 10.0, 15.0, 0, '[]');
             INSERT INTO mmr_entry (id, team_size, mmr, recorded_at, session_id) VALUES
                (1, 2, 899, 500, NULL), (2, 2, 912, 9000, 1), (3, 1, 600, 9000, 1);",
        )
        .unwrap();
        conn
    }

    fn ids(cards: &[SessionCard]) -> Vec<i64> {
        cards.iter().map(|c| c.id).collect()
    }

    #[test]
    fn cards_are_newest_first_with_summaries_and_review_flags() {
        let conn = detail_db();
        let cards = session_cards(&conn, "", 0, 50).unwrap();
        assert_eq!(ids(&cards), [3, 2, 1]);
        let one = &cards[2];
        assert_eq!(
            (
                one.packs,
                one.ranked_matches,
                one.casual_matches,
                one.freeplay_activities
            ),
            (2, 2, 0, 1)
        );
        assert!(one.needs_review);
        assert_eq!(cards[1].casual_matches, 1);
        assert!(!cards[1].needs_review, "reviewed at its end");
        assert!(
            !cards[0].needs_review,
            "the open session is the phone's to review"
        );
        assert_eq!(review_count(&conn).unwrap(), 1);
    }

    #[test]
    fn play_after_the_review_asks_for_a_new_one() {
        let conn = detail_db();
        conn.execute("UPDATE session SET reviewed_at = 25000 WHERE id = 2", [])
            .unwrap();
        assert!(session_cards(&conn, "", 0, 50).unwrap()[1].needs_review);
        assert_eq!(review_count(&conn).unwrap(), 2);
    }

    #[test]
    fn cards_are_paged_and_searched_by_note_pack_name_and_code() {
        let conn = detail_db();
        assert_eq!(ids(&session_cards(&conn, "", 1, 1).unwrap()), [2]);
        assert_eq!(
            ids(&session_cards(&conn, "", -5, 0).unwrap()),
            [3],
            "clamped"
        );
        assert_eq!(ids(&session_cards(&conn, " WARM ", 0, 50).unwrap()), [1]);
        assert_eq!(ids(&session_cards(&conn, "wall", 0, 50).unwrap()), [1]);
        assert_eq!(ids(&session_cards(&conn, "9274d4a2", 0, 50).unwrap()), [1]);
        assert_eq!(
            ids(&session_cards(&conn, "aerial shots", 0, 50).unwrap()),
            [3]
        );
        assert!(
            session_cards(&conn, "%", 0, 50).unwrap().is_empty(),
            "LIKE wildcards are literal"
        );
    }

    #[test]
    fn the_detail_has_activities_totals_and_neighbours() {
        let conn = detail_db();
        let d = session_detail(&conn, 1).unwrap().unwrap();
        assert_eq!(d.notes, "warm-up");
        assert!(d.needs_review);
        assert_eq!((d.previous_id, d.next_id), (None, Some(2)));
        assert_eq!(d.activities.len(), 5);
        let free = &d.activities[0];
        assert_eq!(free.kind, Activity::Freeplay);
        assert_eq!(free.freeplay.as_ref().unwrap().max_hit_speed, 118.0);
        let walls = &d.activities[1];
        assert_eq!(walls.pack.as_ref().unwrap().name, "Wall Clears");
        assert_eq!(
            (walls.attempts, walls.makes),
            (3, 2),
            "the skipped make is left out"
        );
        assert!(d.activities[2].pack.is_none());
        let win = d.activities[3].match_summary.as_ref().unwrap();
        assert_eq!(
            (win.result.as_str(), win.arena.as_str(), win.overtime),
            ("win", "Stadium_P", false)
        );
        assert!(d.activities[4].match_summary.as_ref().unwrap().overtime);
        let t = &d.totals;
        assert_eq!(
            (t.attempts, t.makes, t.wins, t.losses, t.freeplay_ms),
            (4, 2, 1, 1, 61000)
        );
        let target = (0.5 * 3.0 + DEFAULT_TARGET_RATE) / 4.0;
        assert!((t.target_rate.unwrap() - target).abs() < 1e-9);
        let two = session_detail(&conn, 2).unwrap().unwrap();
        assert_eq!((two.previous_id, two.next_id), (Some(1), Some(3)));
        assert_eq!(
            (two.totals.wins, two.totals.losses),
            (0, 0),
            "casual results are not counted"
        );
        assert!(session_detail(&conn, 99).unwrap().is_none());
    }

    #[test]
    fn an_mmr_change_compares_with_the_previous_entry_of_the_same_size() {
        let conn = detail_db();
        conn.execute(
            "INSERT INTO mmr_entry (id, team_size, mmr, recorded_at, session_id) VALUES (4, 2, 920, 9000, 1)",
            [],
        )
        .unwrap();
        let mmr = session_detail(&conn, 1).unwrap().unwrap().mmr;
        let rows: Vec<(i64, u8, i32, Option<i32>)> = mmr
            .iter()
            .map(|m| (m.id, m.team_size, m.mmr, m.change))
            .collect();
        assert_eq!(
            rows,
            [
                (2, 2, 912, Some(13)),
                (3, 1, 600, None),
                (4, 2, 920, Some(8)),
            ],
            "the first 1v1 entry has no change; same-millisecond entries order by id"
        );
    }

    #[test]
    fn a_match_row_carries_its_replay_and_its_place_in_history() {
        let conn = open_in_memory().unwrap();
        crate::store::replays::tests::add_match(&conn, 1, 30, true);
        crate::store::replays::tests::add_match(&conn, 2, 20, true);
        let stats = crate::store::replays::ReplayStats {
            touches: 30,
            ..Default::default()
        };
        crate::store::replays::save_stats(&conn, 2, &stats, "v", 0).unwrap();
        let detail = session_detail(&conn, 1).unwrap().unwrap();
        let summaries: Vec<&MatchSummary> = detail
            .activities
            .iter()
            .filter_map(|a| a.match_summary.as_ref())
            .collect();
        assert_eq!(summaries[0].match_id, 1);
        assert_eq!(summaries[0].history_place, 2);
        assert_eq!(summaries[0].replay, None);
        assert_eq!(summaries[1].replay.as_ref().map(|r| r.touches), Some(30));
    }

    #[test]
    fn cards_for_given_sessions_come_newest_first() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at) VALUES (1, 100), (2, 200), (3, 300);",
        )
        .unwrap();
        let ids: Vec<i64> = session_cards_in(&conn, &[1, 3, 99])
            .unwrap()
            .iter()
            .map(|c| c.id)
            .collect();
        assert_eq!(ids, [3, 1]);
    }

    #[test]
    fn free_play_under_a_minute_is_hidden_but_kept() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 900000);
             INSERT INTO block (id, session_id, kind, started_at, ended_at) VALUES
               (1, 1, 'freeplay', 0, 30000),
               (2, 1, 'freeplay', 60000, 360000),
               (3, 1, 'freeplay', 400000, NULL);",
        )
        .unwrap();
        let detail = session_detail(&conn, 1).unwrap().unwrap();
        let ids: Vec<i64> = detail.activities.iter().map(|a| a.id).collect();
        assert_eq!(ids, [2, 3], "a short one goes; an open one stays");
        let card = &session_cards(&conn, "", 0, 10).unwrap()[0];
        assert_eq!(card.freeplay_activities, 2);
        let kept: i64 = conn
            .query_row("SELECT COUNT(*) FROM block", [], |r| r.get(0))
            .unwrap();
        assert_eq!(kept, 3);
    }
}
