//! Pack management for the desktop: code normalisation, add / edit / hide, list and detail.

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::store::attempt_sql::{counted, made};
use crate::store::edits::{all_pack_stats, clean_notes, EditError, PackRow, PackStats};
use crate::store::scoring::rescore_pack;

pub const CODE_CHARS: usize = 16;
pub const MAX_NAME_CHARS: usize = 80;
pub const MAX_TAGS: usize = 20;
pub const SHOTS: std::ops::RangeInclusive<i64> = 1..=100;
pub const TARGET: std::ops::RangeInclusive<f64> = 0.01..=1.0;

/// Upper case, letters and digits only: how codes are compared and searched.
pub fn code_key(raw: &str) -> String {
    raw.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_uppercase())
        .collect()
}

/// Accepts a code typed any way (case, spaces, dashes) and returns `XXXX-XXXX-XXXX-XXXX`.
pub fn normalize_pack_code(raw: &str) -> Result<String, EditError> {
    let key: String = raw
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '-')
        .map(|c| c.to_ascii_uppercase())
        .collect();
    if !key.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(EditError::Invalid(
            "a pack code uses only 0–9 and A–F".into(),
        ));
    }
    let n = key.len(); // ASCII only past the check above
    if n != CODE_CHARS {
        return Err(EditError::Invalid(format!(
            "the code has {n} of {CODE_CHARS} characters"
        )));
    }
    Ok([&key[0..4], &key[4..8], &key[8..12], &key[12..16]].join("-"))
}

/// A pack as the desktop's Add / Edit window sends it.
#[derive(Debug, Clone, PartialEq, Deserialize, TS)]
#[serde(deny_unknown_fields)]
pub struct PackInput {
    pub code: String,
    pub name: String,
    /// 0.01–1.
    pub target_rate: f64,
    pub shot_count: Option<i64>,
    pub tags: Vec<String>,
    pub notes: String,
    /// Goalie pack: a shot is made unless the ball goes in.
    #[serde(default)]
    pub goalie: bool,
}

struct CleanPack {
    code: String,
    name: String,
    target_rate: f64,
    shot_count: Option<i64>,
    tags_json: String,
    notes: String,
    goalie: bool,
}

fn clean(input: &PackInput) -> Result<CleanPack, EditError> {
    let code = normalize_pack_code(&input.code)?;
    let name = input.name.trim().to_owned();
    let len = name.chars().count();
    if len == 0 || len > MAX_NAME_CHARS {
        return Err(EditError::Invalid(format!(
            "the name needs 1 to {MAX_NAME_CHARS} characters"
        )));
    }
    if !TARGET.contains(&input.target_rate) {
        return Err(EditError::Invalid(
            "the target must be between 1% and 100%".into(),
        ));
    }
    if let Some(n) = input.shot_count {
        if !SHOTS.contains(&n) {
            return Err(EditError::Invalid("shots must be between 1 and 100".into()));
        }
    }
    let tags: Vec<String> = input
        .tags
        .iter()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
        .collect();
    if tags.len() > MAX_TAGS {
        return Err(EditError::Invalid(format!("at most {MAX_TAGS} tags")));
    }
    Ok(CleanPack {
        code,
        name,
        target_rate: input.target_rate,
        shot_count: input.shot_count,
        tags_json: serde_json::to_string(&tags).expect("a list of strings serializes"),
        notes: clean_notes(&input.notes)?,
        goalie: input.goalie,
    })
}

fn refuse_taken(conn: &Connection, code: &str, except: Option<i64>) -> Result<(), EditError> {
    let taken: Option<i64> = conn
        .query_row("SELECT id FROM pack WHERE code = ?1", params![code], |r| {
            r.get(0)
        })
        .optional()?;
    match taken {
        Some(id) if Some(id) != except => Err(EditError::Conflict(format!(
            "a pack with the code {code} already exists"
        ))),
        _ => Ok(()),
    }
}

pub fn add_pack(conn: &Connection, input: &PackInput) -> Result<i64, EditError> {
    let p = clean(input)?;
    refuse_taken(conn, &p.code, None)?;
    conn.execute(
        "INSERT INTO pack (code, name, tags, shot_count, target_rate, notes, goalie)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            p.code,
            p.name,
            p.tags_json,
            p.shot_count,
            p.target_rate,
            p.notes,
            p.goalie
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn update_pack(conn: &Connection, id: i64, input: &PackInput) -> Result<(), EditError> {
    let Some(old) = crate::store::edits::pack(conn, id)? else {
        return Err(EditError::NotFound("pack"));
    };
    let p = clean(input)?;
    refuse_taken(conn, &p.code, Some(id))?;
    conn.execute(
        "UPDATE pack SET code = ?1, name = ?2, tags = ?3, shot_count = ?4, target_rate = ?5,
                notes = ?6, goalie = ?7
         WHERE id = ?8",
        params![
            p.code,
            p.name,
            p.tags_json,
            p.shot_count,
            p.target_rate,
            p.notes,
            p.goalie,
            id
        ],
    )?;
    rescore_pack(conn, id, old.goalie, p.goalie)?;
    Ok(())
}

pub fn set_pack_hidden(conn: &Connection, id: i64, hidden: bool) -> Result<(), EditError> {
    let changed = conn.execute(
        "UPDATE pack SET hidden = ?1 WHERE id = ?2",
        params![hidden, id],
    )?;
    if changed == 0 {
        return Err(EditError::NotFound("pack"));
    }
    Ok(())
}

pub const BEST_SESSION_MIN_ATTEMPTS: i64 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PackSort {
    Recent,
    Name,
    MakeRate,
    MostPlayed,
}

/// Name and tags contain the query (any case), or the code does once case and dashes are ignored.
pub fn pack_matches(pack: &PackRow, query: &str) -> bool {
    let q = query.trim().to_lowercase();
    if q.is_empty() {
        return true;
    }
    if pack.name.to_lowercase().contains(&q)
        || pack.tags.iter().any(|t| t.to_lowercase().contains(&q))
    {
        return true;
    }
    let key = code_key(query);
    !key.is_empty() && code_key(&pack.code).contains(&key)
}

fn desc_none_last<T: PartialOrd>(a: Option<T>, b: Option<T>) -> std::cmp::Ordering {
    use std::cmp::Ordering;
    match (a, b) {
        (Some(a), Some(b)) => b.partial_cmp(&a).unwrap_or(Ordering::Equal),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

/// Every pack (hidden ones too, flagged) with its stats, filtered and sorted. The base order is
/// by name, and the sorts are stable, so ties stay alphabetical.
pub fn pack_list(
    conn: &Connection,
    sort: PackSort,
    search: &str,
) -> rusqlite::Result<Vec<PackStats>> {
    let mut packs: Vec<PackStats> = all_pack_stats(conn, true)?
        .into_iter()
        .filter(|p| pack_matches(&p.pack, search))
        .collect();
    match sort {
        PackSort::Name => {}
        PackSort::Recent => {
            packs.sort_by(|a, b| desc_none_last(a.last_trained_at, b.last_trained_at))
        }
        PackSort::MakeRate => packs.sort_by(|a, b| desc_none_last(a.make_rate, b.make_rate)),
        PackSort::MostPlayed => packs.sort_by_key(|p| std::cmp::Reverse(p.attempts)),
    }
    Ok(packs)
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackSessionRow {
    pub session_id: i64,
    pub started_at: i64,
    pub attempts: i64,
    pub makes: i64,
    pub make_rate: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackDetail {
    pub stats: PackStats,
    pub last_session_rate: Option<f64>,
    /// Best rate among sessions with at least `BEST_SESSION_MIN_ATTEMPTS` attempts.
    pub best_session_rate: Option<f64>,
    /// One row per session that played the pack, newest first.
    pub history: Vec<PackSessionRow>,
}

pub fn pack_detail(conn: &Connection, id: i64) -> rusqlite::Result<Option<PackDetail>> {
    let Some(stats) = all_pack_stats(conn, true)?
        .into_iter()
        .find(|p| p.pack.id == id)
    else {
        return Ok(None);
    };
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.started_at, COUNT(*), SUM({made})
         FROM training_attempt a
         JOIN block b ON b.id = a.block_id
         JOIN session s ON s.id = b.session_id
         WHERE b.pack_id = ?1 AND {counted}
         GROUP BY s.id
         ORDER BY s.started_at DESC, s.id DESC",
        made = made("a"),
        counted = counted("a")
    ))?;
    let history: Vec<PackSessionRow> = stmt
        .query_map(params![id], |r| {
            let attempts: i64 = r.get(2)?;
            let makes: i64 = r.get(3)?;
            Ok(PackSessionRow {
                session_id: r.get(0)?,
                started_at: r.get(1)?,
                attempts,
                makes,
                make_rate: makes as f64 / attempts as f64,
            })
        })?
        .collect::<Result<_, _>>()?;
    let best_session_rate = history
        .iter()
        .filter(|h| h.attempts >= BEST_SESSION_MIN_ATTEMPTS)
        .map(|h| h.make_rate)
        .fold(None, |best: Option<f64>, r| {
            Some(best.map_or(r, |b| b.max(r)))
        });
    Ok(Some(PackDetail {
        last_session_rate: history.first().map(|h| h.make_rate),
        best_session_rate,
        history,
        stats,
    }))
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::edits::{pack, EditError};
    use crate::store::open_in_memory;
    use rusqlite::Connection;

    fn input(code: &str, name: &str) -> PackInput {
        PackInput {
            code: code.into(),
            name: name.into(),
            target_rate: 0.65,
            shot_count: None,
            tags: Vec::new(),
            notes: String::new(),
            goalie: false,
        }
    }

    fn count(conn: &Connection) -> i64 {
        conn.query_row("SELECT COUNT(*) FROM pack", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn codes_are_accepted_in_any_case_with_or_without_dashes() {
        for raw in [
            "c7e09e0bb739a899",
            "C7E0-9E0B-B739-A899",
            " c7e0 9e0b-b739 A899\n",
        ] {
            assert_eq!(normalize_pack_code(raw).unwrap(), "C7E0-9E0B-B739-A899");
        }
        assert_eq!(code_key("c7e0-9e0b b739"), "C7E09E0BB739");
    }

    #[test]
    fn a_wrong_code_says_why() {
        let short = normalize_pack_code("C7E0-9E0B").unwrap_err().to_string();
        assert_eq!(short, "the code has 8 of 16 characters");
        let long = normalize_pack_code("C7E0-9E0B-B739-A8991")
            .unwrap_err()
            .to_string();
        assert_eq!(long, "the code has 17 of 16 characters");
        let letter = normalize_pack_code("C7E0-9E0B-B739-A8O9")
            .unwrap_err()
            .to_string();
        assert_eq!(letter, "a pack code uses only 0–9 and A–F");
    }

    #[test]
    fn a_new_pack_is_stored_cleaned_and_a_taken_code_is_refused() {
        let conn = open_in_memory().unwrap();
        let mut new = input("1111222233334444", "  Backboard Saves ");
        new.tags = vec![" walls ".into(), String::new(), "saves".into()];
        new.notes = " watch the bounce ".into();
        new.shot_count = Some(8);
        let id = add_pack(&conn, &new).unwrap();
        let row = pack(&conn, id).unwrap().unwrap();
        assert_eq!(row.code, "1111-2222-3333-4444");
        assert_eq!(row.name, "Backboard Saves");
        assert_eq!(row.tags, ["walls", "saves"]);
        assert_eq!(row.notes, "watch the bounce");
        assert_eq!(row.shot_count, Some(8));
        assert!(!row.hidden);
        let taken = add_pack(&conn, &input("c7e0 9e0b b739 a899", "Copy")).unwrap_err();
        assert!(matches!(taken, EditError::Conflict(_)));
        assert_eq!(
            taken.to_string(),
            "a pack with the code C7E0-9E0B-B739-A899 already exists"
        );
    }

    #[test]
    fn invalid_pack_fields_write_nothing() {
        let conn = open_in_memory().unwrap();
        let before = count(&conn);
        let code = "1111-2222-3333-4444";
        let mut bad = vec![input(code, ""), input(code, &"x".repeat(81))];
        let mut target = input(code, "T");
        target.target_rate = 0.0;
        bad.push(target);
        let mut shots = input(code, "S");
        shots.shot_count = Some(101);
        bad.push(shots);
        let mut tags = input(code, "Tags");
        tags.tags = (0..21).map(|i| format!("t{i}")).collect();
        bad.push(tags);
        for pack in &bad {
            assert!(matches!(add_pack(&conn, pack), Err(EditError::Invalid(_))));
        }
        assert_eq!(count(&conn), before);
    }

    #[test]
    fn a_pack_is_edited_and_hidden() {
        let conn = open_in_memory().unwrap();
        let mut edit = input("9274d4a2967b0d7a", "Wall Clears 2");
        edit.target_rate = 0.8;
        let walls = pack(&conn, 3).unwrap().unwrap();
        update_pack(&conn, walls.id, &edit).unwrap();
        let row = pack(&conn, walls.id).unwrap().unwrap();
        assert_eq!((row.name.as_str(), row.target_rate), ("Wall Clears 2", 0.8));
        let other_code = input("C7E0-9E0B-B739-A899", "Steal");
        assert!(matches!(
            update_pack(&conn, walls.id, &other_code),
            Err(EditError::Conflict(_))
        ));
        assert!(matches!(
            update_pack(&conn, 999, &edit),
            Err(EditError::NotFound("pack"))
        ));
        set_pack_hidden(&conn, walls.id, true).unwrap();
        assert!(pack(&conn, walls.id).unwrap().unwrap().hidden);
        set_pack_hidden(&conn, walls.id, false).unwrap();
        assert!(!pack(&conn, walls.id).unwrap().unwrap().hidden);
        assert!(matches!(
            set_pack_hidden(&conn, 999, true),
            Err(EditError::NotFound("pack"))
        ));
    }

    /// `n` attempts of which the first `makes` are makes, plus one skipped make that must not count.
    fn attempts(conn: &Connection, block: i64, n: i64, makes: i64, at: i64) {
        for seq in 0..n {
            let outcome = if seq < makes { "make" } else { "miss" };
            conn.execute(
                "INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome)
                 VALUES (?1, ?2, ?3, 3000, ?4)",
                rusqlite::params![block, seq, at + seq, outcome],
            )
            .unwrap();
        }
        conn.execute(
            "INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome, skipped)
             VALUES (?1, 1000, ?2, 3000, 'make', 1)",
            rusqlite::params![block, at + 999],
        )
        .unwrap();
    }

    /// Pack 3 over three sessions: 7/10, 3/3 (too few for "best"), 6/12 (newest).
    fn history_db() -> Connection {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            "INSERT INTO session (id, started_at, ended_at) VALUES
                (1, 0, 900), (2, 1000, 1900), (3, 2000, 2900);
             INSERT INTO block (id, session_id, kind, started_at, ended_at, pack_id) VALUES
                (1, 1, 'training', 0, 900, 3), (2, 2, 'training', 1000, 1900, 3),
                (3, 3, 'training', 2000, 2900, 3), (4, 3, 'training', 2000, 2900, 5);",
        )
        .unwrap();
        attempts(&conn, 1, 10, 7, 0);
        attempts(&conn, 2, 3, 3, 1000);
        attempts(&conn, 3, 12, 6, 2000);
        attempts(&conn, 4, 2, 2, 2000);
        conn
    }

    #[test]
    fn pack_detail_lists_sessions_newest_first_and_best_needs_ten_attempts() {
        let conn = history_db();
        let d = pack_detail(&conn, 3).unwrap().unwrap();
        assert_eq!(
            d.history.iter().map(|h| h.session_id).collect::<Vec<_>>(),
            [3, 2, 1]
        );
        assert_eq!((d.history[0].attempts, d.history[0].makes), (12, 6));
        assert_eq!(d.last_session_rate, Some(0.5));
        assert_eq!(d.best_session_rate, Some(0.7), "3/3 is under 10 attempts");
        assert_eq!((d.stats.attempts, d.stats.makes), (25, 16));
        assert!(pack_detail(&conn, 999).unwrap().is_none());
        let never = pack_detail(&conn, 1).unwrap().unwrap();
        assert!(never.history.is_empty());
        assert_eq!(
            (never.last_session_rate, never.best_session_rate),
            (None, None)
        );
    }

    #[test]
    fn the_pack_list_sorts_searches_and_keeps_hidden_packs() {
        let conn = history_db();
        set_pack_hidden(&conn, 6, true).unwrap();
        let all = pack_list(&conn, PackSort::Name, "").unwrap();
        assert_eq!(all.len(), 11);
        assert!(all.iter().any(|p| p.pack.id == 6 && p.pack.hidden));
        let played = pack_list(&conn, PackSort::MostPlayed, "").unwrap();
        assert_eq!(played[0].pack.id, 3);
        let recent = pack_list(&conn, PackSort::Recent, "").unwrap();
        assert_eq!(
            recent.iter().take(2).map(|p| p.pack.id).collect::<Vec<_>>(),
            [3, 5],
            "pack 3's newest counted attempt (2011) is after pack 5's (2001)"
        );
        let rate = pack_list(&conn, PackSort::MakeRate, "").unwrap();
        assert_eq!(rate[0].pack.id, 5, "2/2");
        assert!(rate.last().unwrap().make_rate.is_none());
        let by_code = pack_list(&conn, PackSort::Name, "c7e09e0b").unwrap();
        assert_eq!(by_code.len(), 1);
        assert_eq!(by_code[0].pack.code, "C7E0-9E0B-B739-A899");
        let by_tag = pack_list(&conn, PackSort::Name, "WALL").unwrap();
        assert!(by_tag
            .iter()
            .all(|p| p.pack.name.to_lowercase().contains("wall")
                || p.pack.tags.iter().any(|t| t.contains("wall"))));
        assert!(!by_tag.is_empty());
    }

    fn outcomes(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT outcome FROM training_attempt ORDER BY id")
            .unwrap();
        stmt.query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    #[test]
    fn switching_a_pack_to_goalie_rescores_its_shots_both_ways() {
        let conn = open_in_memory().unwrap();
        let id = add_pack(&conn, &input("1111222233334444", "Shadow")).unwrap();
        conn.execute_batch(&format!(
            "INSERT INTO session (id, started_at, ended_at) VALUES (1, 0, 100);
             INSERT INTO block (id, session_id, kind, started_at, ended_at, pack_id)
                 VALUES (10, 1, 'training', 0, 100, {id});
             INSERT INTO training_attempt (block_id, seq, started_at, duration_ms, outcome, goal_speed)
                 VALUES (10, 1, 0, 3000, 'make', 40.0), (10, 2, 0, 3000, 'miss', NULL),
                        (10, 3, 0, 3000, 'miss', NULL);"
        ))
        .unwrap();
        let goalie = PackInput {
            goalie: true,
            ..input("1111222233334444", "Shadow")
        };
        update_pack(&conn, id, &goalie).unwrap();
        assert!(pack(&conn, id).unwrap().unwrap().goalie);
        assert_eq!(
            outcomes(&conn),
            ["miss", "make", "make"],
            "the goal let in is the miss"
        );
        update_pack(&conn, id, &goalie).unwrap();
        assert_eq!(
            outcomes(&conn),
            ["miss", "make", "make"],
            "saving again changes nothing"
        );
        update_pack(&conn, id, &input("1111222233334444", "Shadow")).unwrap();
        assert_eq!(outcomes(&conn), ["make", "miss", "miss"]);
    }
}
