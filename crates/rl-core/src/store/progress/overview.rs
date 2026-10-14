//! The Overview (spec §2.3).

use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use super::list::{mode_lanes, ModeLane};
use super::load::{mmr_points, played_packs, session_times};
use super::pack::{pack_points, PackPoint};
use super::rules::{mean, mover_span, rate_change, Weighted};
use super::{MmrPoint, ProgressRange, SessionTime, WEEK_MS};
use crate::store::attempt_sql::{counted, made};
use crate::store::edits::PackRow;

const MOVERS: usize = 4;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ModeMmr {
    pub team_size: u8,
    pub points: Vec<MmrPoint>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Mover {
    pub pack: PackRow,
    /// Make rates (0–1): see `rules::mover_span`.
    pub from: f64,
    pub to: f64,
    pub sessions: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct OverviewProgress {
    /// Start of the range; `None` for All.
    pub start: Option<i64>,
    pub sessions: i64,
    pub played_ms: i64,
    /// Play time per week over the range (All: since the first session).
    pub per_week_ms: i64,
    pub ranked_matches: i64,
    pub wins: i64,
    pub losses: i64,
    /// Counted attempts on packs.
    pub attempts: i64,
    pub make_rate: Option<f64>,
    pub make_rate_change: Option<f64>,
    pub packs_played: i64,
    pub packs_at_target: i64,
    pub mmr: Vec<ModeMmr>,
    pub modes: Vec<ModeLane>,
    /// Up to four packs with the largest change, largest first.
    pub movers: Vec<Mover>,
    /// Oldest first; the UI groups them into weeks.
    pub time: Vec<SessionTime>,
}

pub fn progress_overview(
    conn: &Connection,
    range: ProgressRange,
    now_ms: i64,
) -> rusqlite::Result<OverviewProgress> {
    let start = range.start(now_ms);
    let time = session_times(conn, start)?;
    let played_ms: i64 = time.iter().map(SessionTime::total_ms).sum();
    let span = match range {
        ProgressRange::All => time.first().map_or(0, |t| now_ms - t.at),
        _ => now_ms - start,
    };
    let modes = mode_lanes(conn, start)?;
    let packs = played_packs(conn, start)?;
    let (movers, packs_at_target) = movers(conn, &packs, start)?;
    let training = training_sessions(conn, start)?;
    let mut mmr = Vec::with_capacity(modes.len());
    for lane in &modes {
        mmr.push(ModeMmr {
            team_size: lane.team_size,
            points: mmr_points(conn, lane.team_size, start)?.1,
        });
    }
    Ok(OverviewProgress {
        start: range.shown_start(now_ms),
        sessions: time.len() as i64,
        played_ms,
        per_week_ms: per_week(played_ms, span),
        ranked_matches: modes.iter().map(|m| m.matches).sum(),
        wins: modes.iter().map(|m| m.wins).sum(),
        losses: modes.iter().map(|m| m.losses).sum(),
        attempts: training.iter().map(|w| w.weight as i64).sum(),
        make_rate: mean(&training),
        make_rate_change: rate_change(&training),
        packs_played: packs.len() as i64,
        packs_at_target,
        mmr,
        modes,
        movers,
        time,
    })
}

fn per_week(played_ms: i64, span_ms: i64) -> i64 {
    if played_ms == 0 {
        return 0;
    }
    (i128::from(played_ms) * i128::from(WEEK_MS) / i128::from(span_ms.max(WEEK_MS))) as i64
}

/// The packs that moved most, and how many packs are at or above their target over the range.
fn movers(conn: &Connection, packs: &[PackRow], start: i64) -> rusqlite::Result<(Vec<Mover>, i64)> {
    let mut movers = Vec::new();
    let mut at_target = 0;
    for pack in packs {
        let points = pack_points(conn, pack.id, start)?;
        let makes: i64 = points.iter().map(|p| p.makes).sum();
        let attempts: i64 = points.iter().map(|p| p.attempts).sum();
        if attempts > 0 && makes as f64 / attempts as f64 >= pack.target_rate {
            at_target += 1;
        }
        let weighted: Vec<Weighted> = points.iter().map(PackPoint::weighted).collect();
        if let Some((from, to)) = mover_span(&weighted) {
            movers.push(Mover {
                pack: pack.clone(),
                from,
                to,
                sessions: points.len() as i64,
            });
        }
    }
    movers.sort_by(|a, b| (b.to - b.from).abs().total_cmp(&(a.to - a.from).abs()));
    movers.truncate(MOVERS);
    Ok((movers, at_target))
}

/// Make rate over every pack, one value per session with counted attempts, oldest first.
fn training_sessions(conn: &Connection, start: i64) -> rusqlite::Result<Vec<Weighted>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT COUNT(*), SUM({made})
         FROM training_attempt a
         JOIN block b ON b.id = a.block_id
         JOIN session s ON s.id = b.session_id
         WHERE b.pack_id IS NOT NULL AND {counted} AND s.started_at >= ?1
         GROUP BY s.id ORDER BY s.started_at, s.id",
        made = made("a"),
        counted = counted("a")
    ))?;
    let rows = stmt
        .query_map(params![start], |r| {
            let attempts: i64 = r.get(0)?;
            let makes: i64 = r.get(1)?;
            Ok(Weighted {
                value: makes as f64 / attempts as f64,
                weight: attempts as f64,
            })
        })?
        .collect();
    rows
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::progress::list::progress_list;
    use crate::store::progress::test_db::*;
    use crate::store::progress::DAY_MS;

    /// Three sessions: Uncomfortable Saves (pack 1) 10/20 → 12/20 → 16/20, Wall Clears (pack 3)
    /// 3/10 once, three ranked matches (2v2, 2v2, 1v1) and MMR entries.
    pub(crate) fn seeded() -> Connection {
        let conn = db();
        session(&conn, 1, 40);
        training(&conn, 11, 1, Some(1), 20, 10, 20);
        ranked(&conn, 12, 1, 2, "win", 1, 3, None);
        session(&conn, 2, 20);
        training(&conn, 21, 2, Some(1), 20, 12, 20);
        training(&conn, 22, 2, Some(3), 10, 3, 10);
        ranked(&conn, 23, 2, 2, "loss", 0, 2, Some(300.0));
        session(&conn, 3, 2);
        training(&conn, 31, 3, Some(1), 20, 16, 20);
        ranked(&conn, 32, 3, 1, "win", 2, 4, Some(300.0));
        mmr(&conn, 2, 880, 120, None);
        mmr(&conn, 2, 900, 40, Some(1));
        mmr(&conn, 2, 915, 20, Some(2));
        mmr(&conn, 1, 700, 2, Some(3));
        conn
    }

    #[test]
    fn the_list_has_modes_by_matches_and_packs_by_attempts() {
        let l = progress_list(&seeded(), ProgressRange::Days90, NOW).unwrap();
        assert_eq!((l.sessions, l.played_ms), (3, 85 * 60_000));
        type Lane = (u8, i64, i64, Option<i64>, Option<i64>);
        let modes: Vec<Lane> = l
            .modes
            .iter()
            .map(|m| (m.team_size, m.matches, m.wins, m.mmr_latest, m.mmr_change))
            .collect();
        assert_eq!(
            modes,
            [(2, 2, 1, Some(915), Some(35)), (1, 1, 1, Some(700), None)]
        );
        let packs: Vec<(i64, i64, i64)> = l
            .packs
            .iter()
            .map(|p| (p.pack.id, p.sessions, p.attempts))
            .collect();
        assert_eq!(packs, [(1, 3, 60), (3, 1, 10)]);
        assert!((l.packs[0].rate - 38.0 / 60.0).abs() < 1e-9);
        assert_eq!(l.packs[0].spark, [0.5, 0.6, 0.8]);
    }

    #[test]
    fn the_overview_sums_the_range_and_finds_movers() {
        let o = progress_overview(&seeded(), ProgressRange::Days90, NOW).unwrap();
        assert_eq!(
            (o.sessions, o.ranked_matches, o.wins, o.losses),
            (3, 3, 2, 1)
        );
        assert_eq!(o.attempts, 70);
        assert!((o.make_rate.unwrap() - 41.0 / 70.0).abs() < 1e-9);
        assert_eq!(o.make_rate_change, None, "3 sessions");
        assert_eq!((o.packs_played, o.packs_at_target), (2, 0));
        assert_eq!(o.movers.len(), 1, "Wall Clears has one session");
        assert!((o.movers[0].to - o.movers[0].from - 0.3).abs() < 1e-9);
        assert_eq!(o.time.iter().map(|t| t.ranked_ms).sum::<i64>(), 15 * 60_000);
        assert_eq!(o.start, Some(NOW - 90 * DAY_MS));
        assert_eq!(o.per_week_ms, 85 * 60_000 * 7 / 90);
        let sizes: Vec<u8> = o.mmr.iter().map(|m| m.team_size).collect();
        assert_eq!(sizes, [2, 1]);
    }

    #[test]
    fn casual_and_private_matches_count_as_other_time_only() {
        let conn = seeded();
        conn.execute("UPDATE match_record SET ranked = 0 WHERE block_id = 12", [])
            .unwrap();
        let o = progress_overview(&conn, ProgressRange::Days90, NOW).unwrap();
        assert_eq!(
            (o.ranked_matches, o.time[0].ranked_ms, o.time[0].other_ms),
            (2, 0, 5 * 60_000)
        );
    }

    #[test]
    fn an_empty_database_gives_empty_views() {
        let conn = db();
        let o = progress_overview(&conn, ProgressRange::All, NOW).unwrap();
        assert_eq!((o.sessions, o.played_ms, o.per_week_ms), (0, 0, 0));
        assert_eq!((o.make_rate, o.start), (None, None));
        assert!(o.movers.is_empty() && o.modes.is_empty() && o.time.is_empty());
        let l = progress_list(&conn, ProgressRange::All, NOW).unwrap();
        assert!(l.packs.is_empty() && l.modes.is_empty());
    }
}
