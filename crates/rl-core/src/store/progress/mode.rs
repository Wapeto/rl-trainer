//! One ranked mode over the range (spec §2.4).

use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use super::load::mmr_points;
use super::rules::{mean, mmr_change, rate_change, Weighted};
use super::stats::{ranked_matches, sample, MatchRow, ModeStat};
use super::{MmrPoint, ProgressRange};

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StatPoint {
    pub session_id: i64,
    pub at: i64,
    pub value: f64,
    /// The session's ranked matches of the mode and their results (for the tooltip).
    pub matches: i64,
    pub wins: i64,
    pub losses: i64,
    /// A match of the session was joined late: its time-based stats are partial.
    pub partial: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct StatSeries {
    pub stat: ModeStat,
    /// Sessions with a value for the stat, oldest first.
    pub points: Vec<StatPoint>,
    pub overall: Option<f64>,
    pub change: Option<f64>,
    /// For a stat new in Plan 5 when older matches of the range lack it: the first match with it.
    pub since: Option<i64>,
    /// Matches that count toward the stat; for a Plan 5 stat, matches with the Plan 5 stats recorded.
    pub matches: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ModeProgress {
    pub team_size: u8,
    pub matches: i64,
    pub sessions: i64,
    pub wins: i64,
    pub losses: i64,
    pub overtimes: i64,
    /// MMR entries of the range, oldest first.
    pub mmr: Vec<MmrPoint>,
    pub mmr_latest: Option<i64>,
    pub mmr_change: Option<i64>,
    pub mmr_first: Option<i64>,
    pub mmr_high: Option<i64>,
    /// One series per `ModeStat::ALL`, in that order.
    pub stats: Vec<StatSeries>,
}

pub fn progress_mode(
    conn: &Connection,
    team_size: u8,
    range: ProgressRange,
    now_ms: i64,
) -> rusqlite::Result<ModeProgress> {
    let start = range.start(now_ms);
    let matches = ranked_matches(conn, team_size, start)?;
    let sessions: Vec<&[MatchRow]> = matches
        .chunk_by(|a, b| a.session_id == b.session_id)
        .collect();
    let (before, mmr) = mmr_points(conn, team_size, start)?;
    let values: Vec<i64> = mmr.iter().map(|p| p.mmr).collect();
    let count = |result: &str| matches.iter().filter(|m| m.result == result).count() as i64;
    Ok(ModeProgress {
        team_size,
        matches: matches.len() as i64,
        sessions: sessions.len() as i64,
        wins: count("win"),
        losses: count("loss"),
        overtimes: matches.iter().filter(|m| m.overtime).count() as i64,
        mmr_latest: values.last().copied(),
        mmr_change: mmr_change(before, &values),
        mmr_first: values.first().copied(),
        mmr_high: values.iter().max().copied(),
        stats: ModeStat::ALL
            .iter()
            .map(|&stat| series(stat, &matches, &sessions))
            .collect(),
        mmr,
    })
}

fn series(stat: ModeStat, matches: &[MatchRow], sessions: &[&[MatchRow]]) -> StatSeries {
    let mut weighted = Vec::new();
    let points: Vec<StatPoint> = sessions
        .iter()
        .filter_map(|group| {
            let samples: Vec<Weighted> = group.iter().filter_map(|m| sample(stat, m)).collect();
            let value = mean(&samples)?;
            weighted.push(Weighted {
                value,
                weight: samples.iter().map(|w| w.weight).sum(),
            });
            let result = |r: &str| group.iter().filter(|m| m.result == r).count() as i64;
            Some(StatPoint {
                session_id: group[0].session_id,
                at: group[0].session_at,
                value,
                matches: group.len() as i64,
                wins: result("win"),
                losses: result("loss"),
                partial: group.iter().any(|m| m.partial),
            })
        })
        .collect();
    // A Plan 5 stat counts from the first match with the Plan 5 stats recorded, even when that
    // match has no value for it (no goal of mine, no touch).
    let recorded = |m: &MatchRow| {
        if stat.is_new() {
            m.play_seconds.is_some()
        } else {
            sample(stat, m).is_some()
        }
    };
    let first_with = matches.iter().position(recorded);
    StatSeries {
        stat,
        overall: mean(&weighted),
        change: rate_change(&weighted),
        since: first_with
            .filter(|&i| stat.is_new() && i > 0)
            .map(|i| matches[i].at),
        matches: matches.iter().filter(|m| recorded(m)).count() as i64,
        points,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::progress::test_db::*;
    use crate::store::progress::DAY_MS;

    /// 2v2 over four sessions: two matches before Plan 5, then three with the new stats and one
    /// with an unknown result.
    fn seeded() -> Connection {
        let conn = db();
        session(&conn, 1, 40);
        ranked(&conn, 11, 1, 2, "win", 1, 3, None);
        ranked(&conn, 12, 1, 2, "loss", 0, 2, None);
        session(&conn, 2, 20);
        ranked(&conn, 21, 2, 2, "win", 2, 4, Some(300.0));
        session(&conn, 3, 10);
        ranked(&conn, 31, 3, 2, "loss", 0, 1, Some(300.0));
        session(&conn, 4, 2);
        ranked(&conn, 41, 4, 2, "win", 1, 2, Some(300.0));
        ranked(&conn, 42, 4, 2, "unknown", 0, 0, Some(300.0));
        ranked(&conn, 43, 4, 1, "win", 3, 5, Some(300.0)); // 1v1: another mode
        mmr(&conn, 2, 880, 120, None);
        mmr(&conn, 2, 900, 40, Some(1));
        mmr(&conn, 2, 930, 20, Some(2));
        mmr(&conn, 2, 915, 2, Some(4));
        conn
    }

    fn stat(p: &ModeProgress, stat: ModeStat) -> &StatSeries {
        p.stats.iter().find(|s| s.stat == stat).unwrap()
    }

    #[test]
    fn the_mode_page_counts_results_and_mmr() {
        let p = progress_mode(&seeded(), 2, ProgressRange::Days90, NOW).unwrap();
        assert_eq!((p.matches, p.sessions, p.wins, p.losses), (6, 4, 3, 2));
        assert_eq!(p.stats.len(), ModeStat::ALL.len());
        assert_eq!(
            (p.mmr_latest, p.mmr_change, p.mmr_first, p.mmr_high),
            (Some(915), Some(35), Some(900), Some(930))
        );
        assert_eq!(p.mmr.len(), 3);
    }

    #[test]
    fn per_game_stats_are_per_session_and_overall_per_match() {
        let p = progress_mode(&seeded(), 2, ProgressRange::Days90, NOW).unwrap();
        let goals = stat(&p, ModeStat::Goals);
        let values: Vec<f64> = goals.points.iter().map(|x| x.value).collect();
        assert_eq!(values, [0.5, 2.0, 0.0, 0.5]);
        assert!((goals.overall.unwrap() - 4.0 / 6.0).abs() < 1e-9);
        assert!(goals.change.unwrap().abs() < 1e-9, "(2+0+1)/4 − (1+2+0)/4");
        let win = stat(&p, ModeStat::WinRate);
        assert_eq!(win.points[3].value, 100.0, "the unknown result is left out");
        assert_eq!((win.points[3].wins, win.points[3].losses), (1, 0));
        assert_eq!(win.since, None);
    }

    #[test]
    fn old_matches_never_count_as_zero() {
        let p = progress_mode(&seeded(), 2, ProgressRange::Days90, NOW).unwrap();
        let air = stat(&p, ModeStat::Airborne);
        assert_eq!(
            air.points.len(),
            3,
            "the session before Plan 5 has no point"
        );
        assert!(air.points.iter().all(|x| x.value == 20.0));
        assert_eq!(air.overall, Some(20.0));
        assert_eq!(air.matches, 4);
        let first_new = NOW - 20 * DAY_MS + 600_000; // block 21
        assert_eq!(air.since, Some(first_new));
    }

    #[test]
    fn a_partial_match_marks_its_session() {
        let conn = seeded();
        conn.execute(
            "UPDATE match_record SET metrics_partial = 1 WHERE block_id = 31",
            [],
        )
        .unwrap();
        let p = progress_mode(&conn, 2, ProgressRange::Days90, NOW).unwrap();
        let partial: Vec<bool> = stat(&p, ModeStat::Goals)
            .points
            .iter()
            .map(|x| x.partial)
            .collect();
        assert_eq!(partial, [false, false, true, false]);
    }

    #[test]
    fn a_mode_never_played_is_empty() {
        let p = progress_mode(&seeded(), 3, ProgressRange::Days90, NOW).unwrap();
        assert_eq!((p.matches, p.sessions, p.mmr_latest), (0, 0, None));
        assert!(p
            .stats
            .iter()
            .all(|s| s.points.is_empty() && s.overall.is_none()));
    }

    #[test]
    fn since_follows_recorded_matches_not_a_stat_that_is_empty_for_lack_of_goals() {
        let conn = db();
        session(&conn, 1, 20);
        ranked(&conn, 11, 1, 2, "loss", 0, 2, Some(300.0)); // recorded, no goal of mine
        session(&conn, 2, 10);
        ranked(&conn, 21, 2, 2, "win", 1, 3, Some(300.0));
        conn.execute(
            "UPDATE match_record SET goal_speed_avg = 95 WHERE block_id = 21",
            [],
        )
        .unwrap();
        let p = progress_mode(&conn, 2, ProgressRange::Days90, NOW).unwrap();
        let goal_speed = stat(&p, ModeStat::GoalSpeed);
        assert_eq!(goal_speed.points.len(), 1);
        assert_eq!(
            goal_speed.since, None,
            "every match of the range has the Plan 5 stats"
        );
        assert_eq!(goal_speed.matches, 2);
    }
}
