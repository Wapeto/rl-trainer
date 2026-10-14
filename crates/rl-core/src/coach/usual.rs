//! What a stat is called, how it is shown, which way is better, and a player's usual value.

use serde::Serialize;
use ts_rs::TS;

use crate::store::progress::ModeStat;
use crate::store::replays::ReplayStats;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Better {
    Higher,
    Lower,
    /// Neither direction is good in itself (or it is the outcome): never shown in a review.
    Neither,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum ReviewUnit {
    Percent,
    PerGame,
    Kmh,
    Boost,
    PerMinute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayStat {
    LowBoostTouches,
    BehindBall,
    FirstManShare,
    FiftyWins,
    Whiffs,
}

impl ReplayStat {
    pub const ALL: [Self; 5] = [
        Self::LowBoostTouches,
        Self::BehindBall,
        Self::FirstManShare,
        Self::FiftyWins,
        Self::Whiffs,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Self::LowBoostTouches => "low_boost_touches",
            Self::BehindBall => "behind_ball",
            Self::FirstManShare => "first_man_share",
            Self::FiftyWins => "fifty_wins",
            Self::Whiffs => "whiffs",
        }
    }
}

pub fn replay_meta(stat: ReplayStat) -> (&'static str, ReviewUnit, Better) {
    use ReviewUnit::*;
    match stat {
        ReplayStat::LowBoostTouches => ("Touches under 20 boost", Percent, Better::Lower),
        ReplayStat::BehindBall => ("Time behind the ball", Percent, Better::Higher),
        ReplayStat::FirstManShare => ("Time as first man", Percent, Better::Lower),
        ReplayStat::FiftyWins => ("50/50s won", Percent, Better::Higher),
        ReplayStat::Whiffs => ("Whiffs", PerGame, Better::Lower),
    }
}

pub fn replay_value(stat: ReplayStat, r: &ReplayStats) -> Option<f64> {
    let pct = |part: f64, whole: f64| (whole > 0.0).then(|| part * 100.0 / whole);
    match stat {
        ReplayStat::LowBoostTouches => pct(r.touches_low_boost as f64, r.touches as f64),
        ReplayStat::BehindBall => (r.tracked_seconds > 0.0).then_some(r.pct_behind_ball),
        ReplayStat::FirstManShare => {
            let first = r.secs_first_man?;
            let all = first + r.secs_second_man.unwrap_or(0.0) + r.secs_third_man.unwrap_or(0.0);
            pct(first, all)
        }
        ReplayStat::FiftyWins => pct(r.fifty_wins as f64, (r.fifty_wins + r.fifty_losses) as f64),
        ReplayStat::Whiffs => Some(r.whiffs as f64),
    }
}

pub fn mode_stat_meta(stat: ModeStat) -> (&'static str, ReviewUnit, Better) {
    use Better::*;
    use ModeStat::*;
    use ReviewUnit::*;
    match stat {
        WinRate => ("Win rate", Percent, Neither),
        Goals => ("Goals", PerGame, Higher),
        Assists => ("Assists", PerGame, Higher),
        Saves => ("Saves", PerGame, Higher),
        Shots => ("Shots", PerGame, Higher),
        Score => ("Score", PerGame, Higher),
        ShootingPct => ("Shooting", Percent, Higher),
        GoalSpeed => ("Goal speed", Kmh, Higher),
        EpicSaves => ("Epic saves", PerGame, Higher),
        Touches => ("Touches", PerGame, Higher),
        AerialTouches => ("Aerial touches", PerGame, Higher),
        HitPower => ("Hit power", Kmh, Higher),
        HardHits => ("Hard hits", PerGame, Higher),
        DefensiveThird => ("Touches in defence", Percent, Neither),
        AttackingThird => ("Touches in attack", Percent, Neither),
        Airborne => ("Airborne", Percent, Neither),
        OnWall => ("On the wall", Percent, Neither),
        AvgSpeed => ("Average speed", Kmh, Higher),
        Supersonic => ("Supersonic", Percent, Higher),
        AvgBoost => ("Average boost", Boost, Higher),
        ZeroBoost => ("Time at 0 boost", Percent, Lower),
        FullBoost => ("Time at full boost", Percent, Neither),
        Boosting => ("Time boosting", Percent, Neither),
        BoostPerMinute => ("Boost used", PerMinute, Neither),
        BigPads => ("Big pads", PerGame, Higher),
        DemosGiven => ("Demos", PerGame, Higher),
        DemosTaken => ("Demoed", PerGame, Lower),
        ShareScore => ("Share of team score", Percent, Higher),
        ShareGoals => ("Share of team goals", Percent, Higher),
        ShareSaves => ("Share of team saves", Percent, Higher),
        Possession => ("Possession", Percent, Higher),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Baseline {
    pub mean: f64,
    pub sd: f64,
    pub n: usize,
}

/// Mean and population standard deviation; `None` under 2 values or with no spread.
pub fn baseline(values: &[f64]) -> Option<Baseline> {
    let n = values.len();
    if n < 2 {
        return None;
    }
    let mean = values.iter().sum::<f64>() / n as f64;
    let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / n as f64;
    let sd = var.sqrt();
    (sd > 1e-9).then_some(Baseline { mean, sd, n })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_baseline_needs_two_values_and_spread() {
        assert_eq!(baseline(&[3.0]), None);
        assert_eq!(baseline(&[2.0, 2.0, 2.0]), None, "no spread, no score");
        let b = baseline(&[2.0, 4.0, 4.0, 4.0, 5.0, 5.0, 7.0, 9.0]).unwrap();
        assert!((b.mean - 5.0).abs() < 1e-9);
        assert!((b.sd - 2.0).abs() < 1e-9);
        assert_eq!(b.n, 8);
    }

    #[test]
    fn replay_values_are_ratios_of_my_match() {
        let r = ReplayStats {
            touches: 36,
            touches_low_boost: 9,
            fifty_wins: 3,
            fifty_losses: 1,
            secs_first_man: Some(150.0),
            secs_second_man: Some(150.0),
            secs_third_man: Some(0.0),
            ..ReplayStats::default()
        };
        assert_eq!(replay_value(ReplayStat::LowBoostTouches, &r), Some(25.0));
        assert_eq!(replay_value(ReplayStat::FiftyWins, &r), Some(75.0));
        assert_eq!(replay_value(ReplayStat::FirstManShare, &r), Some(50.0));
        let solo = ReplayStats::default();
        assert_eq!(
            replay_value(ReplayStat::FirstManShare, &solo),
            None,
            "1v1 has no roles"
        );
        assert_eq!(
            replay_value(ReplayStat::LowBoostTouches, &solo),
            None,
            "no touches"
        );
    }

    #[test]
    fn the_outcome_and_neutral_stats_are_never_judged() {
        assert_eq!(mode_stat_meta(ModeStat::WinRate).2, Better::Neither);
        assert_eq!(mode_stat_meta(ModeStat::Airborne).2, Better::Neither);
        assert_eq!(mode_stat_meta(ModeStat::DemosTaken).2, Better::Lower);
        assert_eq!(mode_stat_meta(ModeStat::ShootingPct).2, Better::Higher);
    }
}
