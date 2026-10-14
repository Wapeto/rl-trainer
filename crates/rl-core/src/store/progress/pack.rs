//! One pack's make rate session by session (spec §2.5).

use std::collections::{HashMap, VecDeque};

use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use super::rules::{rate_change, Weighted};
use super::ProgressRange;
use crate::store::attempt_sql::{counted, made};
use crate::store::edits::{pack as pack_row, PackRow};
use crate::store::packs::BEST_SESSION_MIN_ATTEMPTS;

/// Sessions in the dashed average line.
const AVERAGE_OVER: usize = 3;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackPoint {
    pub session_id: i64,
    /// When the session started.
    pub at: i64,
    pub makes: i64,
    pub attempts: i64,
    /// Time on the pack in that session.
    pub ms: i64,
    /// makes ÷ attempts, 0–1.
    pub rate: f64,
    /// Under 10 attempts: drawn hollow, left out of the average line and of the best session.
    pub thin: bool,
    /// Make rate over this session and the two previous ones that are not thin; `None` when thin.
    pub average: Option<f64>,
}

impl PackPoint {
    pub(crate) fn weighted(&self) -> Weighted {
        Weighted {
            value: self.rate,
            weight: self.attempts as f64,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackProgress {
    pub pack: PackRow,
    /// Oldest first.
    pub points: Vec<PackPoint>,
    pub makes: i64,
    pub attempts: i64,
    pub ms: i64,
    /// All makes ÷ all attempts in the range.
    pub rate: Option<f64>,
    /// The latest session's rate minus the first one's in the range (2+ sessions).
    pub latest_change: Option<f64>,
    /// Last 3 sessions minus first 3 (4+ sessions).
    pub change: Option<f64>,
    pub best: Option<PackPoint>,
}

/// One point per session of the range that has counted attempts on the pack, oldest first.
pub(crate) fn pack_points(
    conn: &Connection,
    pack_id: i64,
    start: i64,
) -> rusqlite::Result<Vec<PackPoint>> {
    let mut times = conn.prepare(
        "SELECT b.session_id, SUM(COALESCE(b.ended_at, b.started_at) - b.started_at)
         FROM block b JOIN session s ON s.id = b.session_id
         WHERE b.pack_id = ?1 AND s.started_at >= ?2
         GROUP BY b.session_id",
    )?;
    let ms: HashMap<i64, i64> = times
        .query_map(params![pack_id, start], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    let mut stmt = conn.prepare(&format!(
        "SELECT s.id, s.started_at, COUNT(*), SUM({made})
         FROM training_attempt a
         JOIN block b ON b.id = a.block_id
         JOIN session s ON s.id = b.session_id
         WHERE b.pack_id = ?1 AND {counted} AND s.started_at >= ?2
         GROUP BY s.id ORDER BY s.started_at, s.id",
        made = made("a"),
        counted = counted("a")
    ))?;
    let rows: Vec<(i64, i64, i64, i64)> = stmt
        .query_map(params![pack_id, start], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut recent: VecDeque<(i64, i64)> = VecDeque::new();
    Ok(rows
        .into_iter()
        .map(|(session_id, at, attempts, makes)| {
            let thin = attempts < BEST_SESSION_MIN_ATTEMPTS;
            let average = (!thin).then(|| {
                recent.push_back((makes, attempts));
                if recent.len() > AVERAGE_OVER {
                    recent.pop_front();
                }
                let (m, a) = recent.iter().fold((0, 0), |(m, a), (x, y)| (m + x, a + y));
                m as f64 / a as f64
            });
            PackPoint {
                session_id,
                at,
                makes,
                attempts,
                ms: ms.get(&session_id).copied().unwrap_or(0),
                rate: makes as f64 / attempts as f64,
                thin,
                average,
            }
        })
        .collect())
}

pub fn progress_pack(
    conn: &Connection,
    pack_id: i64,
    range: ProgressRange,
    now_ms: i64,
) -> rusqlite::Result<Option<PackProgress>> {
    let Some(pack) = pack_row(conn, pack_id)? else {
        return Ok(None);
    };
    let points = pack_points(conn, pack_id, range.start(now_ms))?;
    let makes: i64 = points.iter().map(|p| p.makes).sum();
    let attempts: i64 = points.iter().map(|p| p.attempts).sum();
    let weighted: Vec<Weighted> = points.iter().map(PackPoint::weighted).collect();
    let latest_change = match points.as_slice() {
        [first, .., last] => Some(last.rate - first.rate),
        _ => None,
    };
    let best = points
        .iter()
        .filter(|p| !p.thin)
        .max_by(|a, b| a.rate.total_cmp(&b.rate))
        .cloned();
    Ok(Some(PackProgress {
        rate: (attempts > 0).then(|| makes as f64 / attempts as f64),
        change: rate_change(&weighted),
        ms: points.iter().map(|p| p.ms).sum(),
        latest_change,
        best,
        makes,
        attempts,
        pack,
        points,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::progress::test_db::*;

    #[test]
    fn points_are_per_session_oldest_first_and_thin_sessions_stay_out_of_averages() {
        let conn = db();
        session(&conn, 1, 40);
        training(&conn, 11, 1, Some(1), 20, 10, 15);
        session(&conn, 2, 20);
        training(&conn, 21, 2, Some(1), 8, 6, 5);
        session(&conn, 3, 5);
        training(&conn, 31, 3, Some(1), 30, 24, 20);
        session(&conn, 4, 100); // before the 90-day range
        training(&conn, 41, 4, Some(1), 50, 50, 30);
        let p = progress_pack(&conn, 1, ProgressRange::Days90, NOW)
            .unwrap()
            .unwrap();
        let rows: Vec<(i64, i64, i64, bool)> = p
            .points
            .iter()
            .map(|x| (x.session_id, x.makes, x.attempts, x.thin))
            .collect();
        assert_eq!(
            rows,
            [(1, 10, 20, false), (2, 6, 8, true), (3, 24, 30, false)]
        );
        assert_eq!(p.points[0].ms, 15 * 60_000);
        assert_eq!((p.makes, p.attempts), (40, 58));
        assert_eq!(
            p.points[1].average, None,
            "thin sessions are out of the average line"
        );
        assert!((p.points[2].average.unwrap() - 34.0 / 50.0).abs() < 1e-9);
        assert_eq!(
            p.best.as_ref().map(|b| b.session_id),
            Some(3),
            "the thin 75% session is not the best"
        );
        assert!((p.latest_change.unwrap() - 0.3).abs() < 1e-9);
        assert_eq!(p.change, None, "fewer than 4 sessions");
    }

    #[test]
    fn four_sessions_give_a_change_and_all_reaches_older_ones() {
        let conn = db();
        for (id, days, makes) in [(1, 30, 4), (2, 20, 5), (3, 10, 6), (4, 2, 8)] {
            session(&conn, id, days);
            training(&conn, id * 10 + 1, id, Some(1), 10, makes, 10);
        }
        session(&conn, 5, 200);
        training(&conn, 51, 5, Some(1), 10, 1, 10);
        let p = progress_pack(&conn, 1, ProgressRange::Days90, NOW)
            .unwrap()
            .unwrap();
        assert!((p.change.unwrap() - (19.0 / 30.0 - 0.5)).abs() < 1e-9);
        let all = progress_pack(&conn, 1, ProgressRange::All, NOW)
            .unwrap()
            .unwrap();
        assert_eq!(all.points.len(), 5);
    }

    #[test]
    fn a_session_with_only_skipped_attempts_has_no_point() {
        let conn = db();
        session(&conn, 1, 3);
        training(&conn, 11, 1, Some(1), 0, 0, 5);
        let p = progress_pack(&conn, 1, ProgressRange::Days30, NOW)
            .unwrap()
            .unwrap();
        assert!(p.points.is_empty());
        assert_eq!((p.rate, p.latest_change, p.best), (None, None, None));
    }

    #[test]
    fn an_unknown_pack_is_none() {
        assert!(progress_pack(&db(), 999, ProgressRange::All, NOW)
            .unwrap()
            .is_none());
    }
}
