//! Coach fixtures for the UI tests: a session review, the replays to save, the replay status.

use rl_core::coach::review::{ModeReview, ReviewItem, SessionReview, TrainingItem};
use rl_core::coach::usual::ReviewUnit;
use rl_core::store::replays::{PlayerReplayStats, ReplayCounts, ReplayPlayer, ReplayToSave};

use crate::desktop::ReplayStatus;

/// The desktop fixtures' session start (`D0` in `ts_export.rs`).
const D0: i64 = 1_791_489_240_000;

pub(crate) fn to_save() -> Vec<ReplayToSave> {
    vec![ReplayToSave {
        match_id: 21,
        session_id: 12,
        started_at: D0 + 77 * 60_000,
        ranked: true,
        team_size: Some(1),
        result: "loss".into(),
        team_score: 3,
        opponent_score: 8,
        place: 2,
    }]
}

fn review_item(
    key: &str,
    label: &str,
    value: f64,
    usual: f64,
    spread: f64,
    better: bool,
) -> ReviewItem {
    ReviewItem {
        key: key.into(),
        label: label.into(),
        unit: ReviewUnit::Percent,
        value,
        usual,
        better,
        spread,
        covered: 1,
        of: 3,
    }
}

pub(crate) fn review() -> SessionReview {
    SessionReview {
        session_id: 12,
        training: vec![TrainingItem {
            pack_id: Some(11),
            pack_name: Some("Aerial Shots - Pass".into()),
            started_at: D0 + 12 * 60_000,
            makes: 32,
            counted: 159,
            quick_resets: 10,
            make_rate: Some(32.0 / 159.0),
            target_rate: 0.65,
            best_makes: Some(8),
            best_at: Some(D0 + 19 * 60_000),
            dropped_at: Some(D0 + 32 * 60_000),
            bars: vec![
                0.9, 0.7, 0.4, 0.25, 0.1, 0.15, 0.2, 0.1, 0.3, 0.15, 0.12, 0.22,
            ],
        }],
        modes: vec![
            ModeReview {
                team_size: 1,
                matches: 2,
                earlier: 3,
                items: vec![],
            },
            ModeReview {
                team_size: 2,
                matches: 3,
                earlier: 20,
                items: vec![
                    review_item(
                        "low_boost_touches",
                        "Touches under 20 boost",
                        49.0,
                        30.0,
                        8.0,
                        false,
                    ),
                    review_item("fifty_wins", "50/50s won", 71.0, 55.0, 9.0, true),
                ],
            },
        ],
        to_save: to_save(),
        replay_added_at: Some(D0 + 130 * 60_000),
    }
}

pub(crate) fn replay_status() -> ReplayStatus {
    ReplayStatus {
        folders: vec![
            "C:\\Users\\me\\OneDrive\\Documents\\My Games\\Rocket League\\TAGame\\DemosEpic".into(),
        ],
        counts: ReplayCounts {
            imported: 4,
            unmatched: 0,
            not_mine: 0,
            errors: 1,
            last_error: Some("the replay could not be read: unexpected end of data".into()),
            format_warning: false,
        },
    }
}

/// A 2v2 replay's page: players ("you" are slot 1, made-up names), the winners' benchmark and three insights.
pub(crate) fn match_replay() -> crate::desktop::MatchReplay {
    let stats_of = |goals: f64, behind: f64, whiffs: f64, first: Option<f64>| PlayerReplayStats {
        tracked_seconds: Some(300.0),
        score: Some(200.0 + goals * 100.0),
        goals: Some(goals),
        assists: Some(1.0),
        saves: Some(2.0),
        shots: Some(3.0),
        pct_behind_ball: Some(behind),
        pct_in_front_of_ball: Some(100.0 - behind - 5.0),
        secs_first_man: first,
        secs_second_man: first.map(|f| 300.0 - f),
        boost_collected: Some(3200.0),
        big_pads: Some(14.0),
        touches: Some(30.0),
        touches_low_boost: Some(9.0),
        fifty_wins: Some(4.0),
        fifty_losses: Some(2.0),
        whiffs: Some(whiffs),
        avg_speed_kmh: Some(54.2),
        avg_distance_to_ball_m: Some(23.4),
        ..PlayerReplayStats::default()
    };
    let p = |slot: i64, team: i64, name: &str, is_me: bool, stats| ReplayPlayer {
        slot,
        team,
        name: name.into(),
        is_me,
        is_bot: false,
        stats,
    };
    let insight = |stat: &str, fact: &str, tip: &str| rl_core::coach::insights::Insight {
        stat: stat.into(),
        fact: fact.into(),
        tip: tip.into(),
    };
    let players = vec![
        p(
            0,
            0,
            "Teammate",
            false,
            stats_of(1.0, 70.0, 2.0, Some(150.0)),
        ),
        p(1, 0, "You", true, stats_of(2.0, 64.0, 3.0, Some(144.0))),
        p(
            2,
            1,
            "Rival A",
            false,
            stats_of(0.0, 58.0, 5.0, Some(160.0)),
        ),
        p(3, 1, "Rival B", false, PlayerReplayStats::default()),
    ];
    let bench = |gap: f64, me: f64, winners_pct: f64| rl_core::coach::benchmark::BenchStat {
        gap,
        separates: gap.abs() >= rl_core::coach::benchmark::MIN_GAP,
        me: Some(me),
        winners_pct,
    };
    let stats = [
        ("boost_collected", bench(0.8, 40.0, 62.0)),
        ("big_pads", bench(0.5, 55.0, 58.0)),
        ("pct_zero_boost", bench(-0.9, 20.0, 64.0)),
        ("boost_used_supersonic", bench(-0.4, 35.0, 57.0)),
        ("pct_full_boost", bench(0.1, 50.0, 51.0)),
        ("touches", bench(0.4, 60.0, 56.0)),
        ("hard_hits", bench(0.7, 30.0, 63.0)),
        ("first_touches", bench(0.5, 52.0, 59.0)),
        ("avg_speed_kmh", bench(0.6, 66.0, 60.0)),
        ("pct_supersonic", bench(0.5, 58.0, 61.0)),
        ("pct_on_ground", bench(-0.35, 45.0, 55.0)),
        ("pct_behind_ball", bench(0.6, 45.0, 60.0)),
        ("pct_in_front_of_ball", bench(-0.6, 48.0, 61.0)),
        ("avg_distance_to_ball_m", bench(0.2, 50.0, 52.0)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect();
    crate::desktop::MatchReplay {
        players,
        benchmark: rl_core::coach::benchmark::Benchmark {
            matches: 20,
            winners: Some(stats_of(1.5, 66.0, 2.0, Some(140.0))),
            losers: Some(stats_of(0.5, 58.0, 4.0, Some(150.0))),
            stats,
        },
        insights: vec![
            insight("pct_zero_boost", "Time at 0 boost: 26% — winners average 13%.", "Route back over the small pads instead of detouring to a corner: two or three of them keep you in the play."),
            insight("secs_first_man", "1st man: 61% of the match, your teammate 38%.", "When your teammate is closer and facing the ball, let them have it and cover."),
            insight("fifty_losses", "50/50s: won 2, lost 7.", "Challenge from the side the ball is leaving, and aim for the corner."),
        ],
    }
}
