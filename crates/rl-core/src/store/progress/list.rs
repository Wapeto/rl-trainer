//! The Progress list column (spec §2.2).

use rusqlite::{params, Connection};
use serde::Serialize;
use ts_rs::TS;

use super::load::{mmr_points, played_packs, session_times};
use super::pack::{pack_points, PackPoint};
use super::rules::{mmr_change, rate_change, Weighted};
use super::{ProgressRange, SessionTime};
use crate::store::edits::PackRow;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ModeLane {
    pub team_size: u8,
    pub matches: i64,
    pub wins: i64,
    pub losses: i64,
    pub mmr_latest: Option<i64>,
    pub mmr_change: Option<i64>,
    /// MMR entries of the range, oldest first.
    pub mmr_spark: Vec<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct PackLane {
    pub pack: PackRow,
    pub sessions: i64,
    pub attempts: i64,
    /// Make rate over the range, 0–1.
    pub rate: f64,
    pub change: Option<f64>,
    /// Per-session make rates, oldest first.
    pub spark: Vec<f64>,
    pub last_played: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ProgressList {
    pub sessions: i64,
    pub played_ms: i64,
    /// Most ranked matches first.
    pub modes: Vec<ModeLane>,
    /// Most attempts first, then by name.
    pub packs: Vec<PackLane>,
}

pub fn progress_list(
    conn: &Connection,
    range: ProgressRange,
    now_ms: i64,
) -> rusqlite::Result<ProgressList> {
    let start = range.start(now_ms);
    let time = session_times(conn, start)?;
    let mut packs = Vec::new();
    for pack in played_packs(conn, start)? {
        let points = pack_points(conn, pack.id, start)?;
        let weighted: Vec<Weighted> = points.iter().map(PackPoint::weighted).collect();
        let attempts: i64 = points.iter().map(|p| p.attempts).sum();
        let makes: i64 = points.iter().map(|p| p.makes).sum();
        packs.push(PackLane {
            rate: makes as f64 / attempts.max(1) as f64,
            change: rate_change(&weighted),
            spark: points.iter().map(|p| p.rate).collect(),
            last_played: points.last().map_or(0, |p| p.at),
            sessions: points.len() as i64,
            attempts,
            pack,
        });
    }
    // Stable: ties keep `played_packs`' name order.
    packs.sort_by_key(|p| std::cmp::Reverse(p.attempts));
    Ok(ProgressList {
        sessions: time.len() as i64,
        played_ms: time.iter().map(SessionTime::total_ms).sum(),
        modes: mode_lanes(conn, start)?,
        packs,
    })
}

/// Ranked modes with matches in the range, most matches first.
pub(crate) fn mode_lanes(conn: &Connection, start: i64) -> rusqlite::Result<Vec<ModeLane>> {
    let mut stmt = conn.prepare(
        "SELECT m.team_size, COUNT(*), SUM(m.result = 'win'), SUM(m.result = 'loss')
         FROM match_record m JOIN block b ON b.id = m.block_id
         JOIN session s ON s.id = b.session_id
         WHERE m.ranked = 1 AND m.team_size BETWEEN 1 AND 3 AND s.started_at >= ?1
         GROUP BY m.team_size ORDER BY COUNT(*) DESC, m.team_size",
    )?;
    let rows: Vec<(u8, i64, i64, i64)> = stmt
        .query_map(params![start], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    rows.into_iter()
        .map(|(team_size, matches, wins, losses)| {
            let (before, points) = mmr_points(conn, team_size, start)?;
            let spark: Vec<i64> = points.iter().map(|p| p.mmr).collect();
            Ok(ModeLane {
                team_size,
                matches,
                wins,
                losses,
                mmr_latest: spark.last().copied(),
                mmr_change: mmr_change(before, &spark),
                mmr_spark: spark,
            })
        })
        .collect()
}
