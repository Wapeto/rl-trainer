//! The ranked match stats of the mode page (spec §4.3): what each stat is, how one match counts
//! toward it, and how matches are read.

use rusqlite::{params, Connection, Row};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::rules::Weighted;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ModeStat {
    WinRate,
    Goals,
    Assists,
    Saves,
    Shots,
    Score,
    ShootingPct,
    GoalSpeed,
    EpicSaves,
    Touches,
    AerialTouches,
    HitPower,
    HardHits,
    DefensiveThird,
    AttackingThird,
    Airborne,
    OnWall,
    AvgSpeed,
    Supersonic,
    AvgBoost,
    ZeroBoost,
    FullBoost,
    Boosting,
    BoostPerMinute,
    BigPads,
    DemosGiven,
    DemosTaken,
    ShareScore,
    ShareGoals,
    ShareSaves,
    Possession,
}

impl ModeStat {
    pub const ALL: [Self; 31] = [
        Self::WinRate,
        Self::Goals,
        Self::Assists,
        Self::Saves,
        Self::Shots,
        Self::Score,
        Self::ShootingPct,
        Self::GoalSpeed,
        Self::EpicSaves,
        Self::Touches,
        Self::AerialTouches,
        Self::HitPower,
        Self::HardHits,
        Self::DefensiveThird,
        Self::AttackingThird,
        Self::Airborne,
        Self::OnWall,
        Self::AvgSpeed,
        Self::Supersonic,
        Self::AvgBoost,
        Self::ZeroBoost,
        Self::FullBoost,
        Self::Boosting,
        Self::BoostPerMinute,
        Self::BigPads,
        Self::DemosGiven,
        Self::DemosTaken,
        Self::ShareScore,
        Self::ShareGoals,
        Self::ShareSaves,
        Self::Possession,
    ];

    /// Recorded only since Plan 5: older matches have no value for it.
    pub fn is_new(self) -> bool {
        matches!(
            self,
            Self::GoalSpeed
                | Self::AerialTouches
                | Self::HitPower
                | Self::HardHits
                | Self::DefensiveThird
                | Self::AttackingThird
                | Self::Airborne
                | Self::OnWall
                | Self::AvgSpeed
                | Self::FullBoost
                | Self::Boosting
                | Self::BoostPerMinute
                | Self::BigPads
                | Self::DemosTaken
                | Self::Possession
        )
    }
}

/// One ranked match as the mode page reads it; the Plan 5 columns are `None` for older ones.
#[derive(Debug, Clone, Default)]
pub struct MatchRow {
    pub session_id: i64,
    pub session_at: i64,
    /// When the match's activity started.
    pub at: i64,
    pub result: String,
    pub overtime: bool,
    pub partial: bool,
    pub score: f64,
    pub goals: f64,
    pub assists: f64,
    pub saves: f64,
    pub shots: f64,
    pub touches: f64,
    pub demos: f64,
    pub epic_saves: f64,
    pub avg_boost: f64,
    pub pct_zero_boost: f64,
    pub pct_supersonic: f64,
    pub scoreboard_json: String,
    pub play_seconds: Option<f64>,
    pub hit_avg_speed: Option<f64>,
    pub hard_hits: Option<f64>,
    pub aerial_touches: Option<f64>,
    pub touches_def: Option<f64>,
    pub touches_mid: Option<f64>,
    pub touches_att: Option<f64>,
    pub pct_on_wall: Option<f64>,
    pub pct_airborne: Option<f64>,
    pub avg_speed: Option<f64>,
    pub pct_boosting: Option<f64>,
    pub pct_full_boost: Option<f64>,
    pub boost_used: Option<f64>,
    pub big_pads: Option<f64>,
    pub goal_speed_avg: Option<f64>,
    pub demos_taken: Option<f64>,
    pub pct_possession: Option<f64>,
    pub match_id: i64,
    pub team_size: i64,
}

const MATCH_COLUMNS: &str = "s.id, s.started_at, b.started_at, m.result, m.overtime,
    m.metrics_partial, m.score, m.goals, m.assists, m.saves, m.shots, m.touches, m.demos,
    m.epic_saves, m.avg_boost, m.pct_zero_boost, m.pct_supersonic, m.scoreboard_json,
    m.play_seconds, m.hit_avg_speed, m.hard_hits, m.aerial_touches, m.touches_def,
    m.touches_mid, m.touches_att, m.pct_on_wall, m.pct_airborne, m.avg_speed, m.pct_boosting,
    m.pct_full_boost, m.boost_used, m.big_pads, m.goal_speed_avg, m.demos_taken,
    m.pct_possession, m.id, COALESCE(m.team_size, 0)";

fn match_row(r: &Row) -> rusqlite::Result<MatchRow> {
    Ok(MatchRow {
        session_id: r.get(0)?,
        session_at: r.get(1)?,
        at: r.get(2)?,
        result: r.get(3)?,
        overtime: r.get(4)?,
        partial: r.get(5)?,
        score: r.get(6)?,
        goals: r.get(7)?,
        assists: r.get(8)?,
        saves: r.get(9)?,
        shots: r.get(10)?,
        touches: r.get(11)?,
        demos: r.get(12)?,
        epic_saves: r.get(13)?,
        avg_boost: r.get(14)?,
        pct_zero_boost: r.get(15)?,
        pct_supersonic: r.get(16)?,
        scoreboard_json: r.get(17)?,
        play_seconds: r.get(18)?,
        hit_avg_speed: r.get(19)?,
        hard_hits: r.get(20)?,
        aerial_touches: r.get(21)?,
        touches_def: r.get(22)?,
        touches_mid: r.get(23)?,
        touches_att: r.get(24)?,
        pct_on_wall: r.get(25)?,
        pct_airborne: r.get(26)?,
        avg_speed: r.get(27)?,
        pct_boosting: r.get(28)?,
        pct_full_boost: r.get(29)?,
        boost_used: r.get(30)?,
        big_pads: r.get(31)?,
        goal_speed_avg: r.get(32)?,
        demos_taken: r.get(33)?,
        pct_possession: r.get(34)?,
        match_id: r.get(35)?,
        team_size: r.get(36)?,
    })
}

/// Ranked matches of one team size in sessions of the range, in play order.
pub(crate) fn ranked_matches(
    conn: &Connection,
    team_size: u8,
    start: i64,
) -> rusqlite::Result<Vec<MatchRow>> {
    let sql = format!(
        "SELECT {MATCH_COLUMNS} FROM match_record m
         JOIN block b ON b.id = m.block_id JOIN session s ON s.id = b.session_id
         WHERE m.ranked = 1 AND m.team_size = ?1 AND s.started_at >= ?2
         ORDER BY s.started_at, s.id, b.started_at, m.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![team_size, start], match_row)?
        .collect();
    rows
}

/// The session's ranked matches, every team size, in play order.
pub(crate) fn session_ranked_matches(
    conn: &Connection,
    session_id: i64,
) -> rusqlite::Result<Vec<MatchRow>> {
    let sql = format!(
        "SELECT {MATCH_COLUMNS} FROM match_record m
         JOIN block b ON b.id = m.block_id JOIN session s ON s.id = b.session_id
         WHERE m.ranked = 1 AND b.session_id = ?1 ORDER BY b.started_at, m.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![session_id], match_row)?.collect();
    rows
}

/// Up to `limit` ranked matches of a team size that started before `before`, newest first.
pub(crate) fn ranked_matches_before(
    conn: &Connection,
    team_size: i64,
    before: i64,
    limit: i64,
) -> rusqlite::Result<Vec<MatchRow>> {
    let sql = format!(
        "SELECT {MATCH_COLUMNS} FROM match_record m
         JOIN block b ON b.id = m.block_id JOIN session s ON s.id = b.session_id
         WHERE m.ranked = 1 AND m.team_size = ?1 AND b.started_at < ?2
         ORDER BY b.started_at DESC, m.id DESC LIMIT ?3"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt
        .query_map(params![team_size, before, limit], match_row)?
        .collect();
    rows
}

fn per_game(value: f64) -> Option<Weighted> {
    Some(Weighted { value, weight: 1.0 })
}

/// `part` as a percentage of `whole`, weighted by `whole`.
fn ratio(part: f64, whole: f64) -> Option<Weighted> {
    (whole > 0.0).then(|| Weighted {
        value: part * 100.0 / whole,
        weight: whole,
    })
}

/// A Plan 5 percentage of play time, weighted by play time.
fn timed(value: Option<f64>, m: &MatchRow) -> Option<Weighted> {
    Some(Weighted {
        value: value?,
        weight: m.play_seconds.filter(|s| *s > 0.0)?,
    })
}

fn located_touches(m: &MatchRow) -> Option<f64> {
    Some(m.touches_def? + m.touches_mid? + m.touches_att?)
}

#[derive(Deserialize)]
struct BoardRow {
    team_num: i32,
    #[serde(default)]
    me: bool,
    score: f64,
    goals: f64,
    assists: f64,
    saves: f64,
    shots: f64,
}

struct TeamTotals {
    score: f64,
    goals: f64,
    saves: f64,
}

/// My team's totals from the stored scoreboard. It names players but has no ids: my row is the
/// one flagged `me` (since Plan 6), else the one with my exact stat line. `None` when it is
/// missing or unreadable.
fn team_totals(m: &MatchRow) -> Option<TeamTotals> {
    let rows: Vec<BoardRow> = serde_json::from_str(&m.scoreboard_json).ok()?;
    let mine = rows.iter().find(|r| r.me).or_else(|| {
        rows.iter().find(|r| {
            r.score == m.score
                && r.goals == m.goals
                && r.assists == m.assists
                && r.saves == m.saves
                && r.shots == m.shots
        })
    })?;
    Some(rows.iter().filter(|r| r.team_num == mine.team_num).fold(
        TeamTotals {
            score: 0.0,
            goals: 0.0,
            saves: 0.0,
        },
        |t, r| TeamTotals {
            score: t.score + r.score,
            goals: t.goals + r.goals,
            saves: t.saves + r.saves,
        },
    ))
}

/// How one match counts toward a stat: its value and its weight. `None` when the match has no
/// value for it (a Plan 5 stat on an older match, a shooting % without shots…).
pub(crate) fn sample(stat: ModeStat, m: &MatchRow) -> Option<Weighted> {
    use ModeStat::*;
    match stat {
        WinRate => match m.result.as_str() {
            "win" => per_game(100.0),
            "loss" => per_game(0.0),
            _ => None,
        },
        Goals => per_game(m.goals),
        Assists => per_game(m.assists),
        Saves => per_game(m.saves),
        Shots => per_game(m.shots),
        Score => per_game(m.score),
        ShootingPct => ratio(m.goals, m.shots),
        GoalSpeed => Some(Weighted {
            value: m.goal_speed_avg?,
            weight: m.goals.max(1.0),
        }),
        EpicSaves => per_game(m.epic_saves),
        Touches => per_game(m.touches),
        AerialTouches => per_game(m.aerial_touches?),
        HitPower => per_game(m.hit_avg_speed?),
        HardHits => per_game(m.hard_hits?),
        DefensiveThird => ratio(m.touches_def?, located_touches(m)?),
        AttackingThird => ratio(m.touches_att?, located_touches(m)?),
        Airborne => timed(m.pct_airborne, m),
        OnWall => timed(m.pct_on_wall, m),
        AvgSpeed => timed(m.avg_speed, m),
        // Recorded since Plan 1 without a play time: averaged per match.
        Supersonic => per_game(m.pct_supersonic),
        AvgBoost => per_game(m.avg_boost),
        ZeroBoost => per_game(m.pct_zero_boost),
        FullBoost => timed(m.pct_full_boost, m),
        Boosting => timed(m.pct_boosting, m),
        BoostPerMinute => {
            let seconds = m.play_seconds.filter(|s| *s > 0.0)?;
            Some(Weighted {
                value: m.boost_used? * 60.0 / seconds,
                weight: seconds,
            })
        }
        BigPads => per_game(m.big_pads?),
        DemosGiven => per_game(m.demos),
        DemosTaken => per_game(m.demos_taken?),
        ShareScore => ratio(m.score, team_totals(m)?.score),
        ShareGoals => ratio(m.goals, team_totals(m)?.goals),
        ShareSaves => ratio(m.saves, team_totals(m)?.saves),
        Possession => timed(m.pct_possession, m),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row() -> MatchRow {
        MatchRow {
            result: "win".into(),
            score: 200.0,
            goals: 1.0,
            saves: 1.0,
            shots: 4.0,
            scoreboard_json: r#"[{"name":"Me","team_num":0,"score":200,"goals":1,"assists":0,"saves":1,"shots":4,"demos":0},{"name":"Mate","team_num":0,"score":300,"goals":2,"assists":0,"saves":3,"shots":2,"demos":0},{"name":"Them","team_num":1,"score":900,"goals":9,"assists":0,"saves":0,"shots":9,"demos":0}]"#.into(),
            ..MatchRow::default()
        }
    }

    #[test]
    fn every_stat_has_a_rule_and_old_matches_lack_only_the_new_ones() {
        let old = row();
        for stat in ModeStat::ALL {
            assert_eq!(sample(stat, &old).is_none(), stat.is_new(), "{stat:?}");
        }
    }

    #[test]
    fn ratios_divide_sums_and_shares_use_my_team() {
        let m = row();
        assert_eq!(
            sample(ModeStat::ShootingPct, &m),
            Some(Weighted {
                value: 25.0,
                weight: 4.0
            })
        );
        assert_eq!(
            sample(ModeStat::ShareScore, &m),
            Some(Weighted {
                value: 40.0,
                weight: 500.0
            })
        );
        assert_eq!(
            sample(ModeStat::ShareSaves, &m).map(|w| w.value),
            Some(25.0)
        );
        let unknown = MatchRow {
            result: "unknown".into(),
            ..row()
        };
        assert_eq!(sample(ModeStat::WinRate, &unknown), None);
    }

    #[test]
    fn timed_stats_weigh_by_play_time_and_a_bad_scoreboard_has_no_share() {
        let m = MatchRow {
            play_seconds: Some(300.0),
            pct_airborne: Some(12.0),
            boost_used: Some(3000.0),
            touches_def: Some(3.0),
            touches_mid: Some(5.0),
            touches_att: Some(2.0),
            scoreboard_json: "[300]".into(),
            ..row()
        };
        assert_eq!(
            sample(ModeStat::Airborne, &m),
            Some(Weighted {
                value: 12.0,
                weight: 300.0
            })
        );
        assert_eq!(
            sample(ModeStat::BoostPerMinute, &m).map(|w| w.value),
            Some(600.0)
        );
        assert_eq!(
            sample(ModeStat::DefensiveThird, &m).map(|w| w.value),
            Some(30.0)
        );
        assert_eq!(sample(ModeStat::ShareScore, &m), None);
    }

    #[test]
    fn my_scoreboard_row_is_found_by_its_flag_before_the_stat_line() {
        let m = MatchRow {
            score: 100.0,
            goals: 1.0,
            scoreboard_json: r#"[{"name":"Twin","team_num":0,"score":100,"goals":1,"assists":0,"saves":0,"shots":0,"demos":0},
                                 {"name":"Me","team_num":1,"score":100,"goals":1,"assists":0,"saves":0,"shots":0,"demos":0,"me":true},
                                 {"name":"Mate","team_num":1,"score":300,"goals":2,"assists":0,"saves":0,"shots":0,"demos":0}]"#
                .into(),
            ..MatchRow::default()
        };
        let share = sample(ModeStat::ShareScore, &m).unwrap();
        assert!((share.value - 25.0).abs() < 1e-9, "100 of my team's 400");
    }
}
