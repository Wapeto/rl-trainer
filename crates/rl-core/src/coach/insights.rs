//! What stood out in one match: a fact and a coaching tip (match insights spec §3, winners'
//! benchmark spec §3). Pure: this match's players and the winners of my recent replays in the mode
//! in, insights out.

use serde::Serialize;
use ts_rs::TS;

use super::usual::baseline;
use crate::store::replays::{PlayerReplayStats, ReplayPlayer};

pub(crate) const FIVE_MIN: f64 = 300.0;

/// Counts and seconds shown per 5 minutes (the page divides them by tracked time).
pub const PER_FIVE: [&str; 31] = [
    "secs_first_man",
    "secs_second_man",
    "secs_third_man",
    "became_first_man",
    "boost_collected",
    "boost_used",
    "boost_stolen",
    "big_pads",
    "small_pads",
    "boost_used_supersonic",
    "touches_low_boost",
    "touches",
    "hard_hits",
    "aerial_touches",
    "control_touches",
    "first_touches",
    "fifty_wins",
    "fifty_losses",
    "fifty_neutral",
    "beaten_to_ball",
    "whiffs",
    "flicks",
    "wavedashes",
    "half_flips",
    "speed_flips",
    "demos_inflicted",
    "demos_taken",
    "bumps_inflicted",
    "bumps_taken",
    "passes_completed",
    "passes_received",
];

pub const MAX_INSIGHTS: usize = 3;
/// Against the winners: at least this many sd worse, and this much worse relatively.
const MIN_Z: f64 = 1.0;
const MIN_REL_WINNERS: f64 = 0.2;
/// Against the match: the worst of all, and this much worse than the others' mean.
const MIN_REL_MATCH: f64 = 0.3;
/// Scores are capped, so one huge gap does not swamp the rest.
const MAX_SCORE: f64 = 3.0;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct Insight {
    /// The page row it points to.
    pub stat: String,
    pub fact: String,
    pub tip: String,
}

#[derive(Clone, Copy)]
enum Unit {
    Pct,
    PerFive,
}

/// A stat where higher is worse, compared with the winners or with the match (spec §3.1).
struct Rule {
    stat: &'static str,
    label: &'static str,
    unit: Unit,
    value: fn(&PlayerReplayStats) -> Option<f64>,
    /// Extra condition on this match only (demos: at least 2).
    gate: fn(&PlayerReplayStats) -> bool,
    tip: &'static str,
}

fn per_five(s: &PlayerReplayStats, v: Option<f64>) -> Option<f64> {
    let t = s.tracked_seconds.filter(|t| *t > 0.0)?;
    Some(v? * FIVE_MIN / t)
}

fn always(_: &PlayerReplayStats) -> bool {
    true
}

const COMPARED: [Rule; 6] = [
    Rule {
        stat: "pct_zero_boost",
        label: "Time at 0 boost",
        unit: Unit::Pct,
        value: |s| s.pct_zero_boost,
        gate: always,
        tip: "Route back over the small pads instead of detouring to a corner: two or three of them keep you in the play.",
    },
    Rule {
        stat: "boost_used_supersonic",
        label: "Boost used at full speed per 5 min",
        unit: Unit::PerFive,
        value: |s| per_five(s, s.boost_used_supersonic),
        gate: always,
        tip: "At full speed, tap boost instead of holding it: holding adds nothing.",
    },
    Rule {
        stat: "touches_low_boost",
        label: "Touches under 20 boost",
        unit: Unit::Pct,
        value: |s| match (s.touches_low_boost, s.touches) {
            (Some(low), Some(all)) if all > 0.0 => Some(low * 100.0 / all),
            _ => None,
        },
        gate: always,
        tip: "With low boost, shadow or fake instead of challenging; go when the attacker pushes the ball too far ahead.",
    },
    Rule {
        stat: "pct_in_front_of_ball",
        label: "In front of the ball",
        unit: Unit::Pct,
        value: |s| s.pct_in_front_of_ball,
        gate: always,
        tip: "Rotate back toward the back post, away from the ball's side.",
    },
    Rule {
        stat: "demos_taken",
        label: "Demos taken per 5 min",
        unit: Unit::PerFive,
        value: |s| per_five(s, s.demos_taken),
        gate: |s| s.demos_taken.is_some_and(|d| d >= 2.0),
        tip: "Vary your line when rotating: straight, ball-watching paths are the easiest to demo.",
    },
    Rule {
        stat: "whiffs",
        label: "Whiffs per 5 min",
        unit: Unit::PerFive,
        value: |s| per_five(s, s.whiffs),
        gate: always,
        tip: "Last back and unsure? Shadow instead of going for it.",
    },
];

fn show(unit: Unit, v: f64) -> String {
    match unit {
        Unit::Pct => format!("{}%", v.round()),
        Unit::PerFive => format!("{v:.1}"),
    }
}

fn insight(stat: &str, fact: String, tip: &str) -> Insight {
    Insight {
        stat: stat.into(),
        fact,
        tip: tip.into(),
    }
}

fn compared(
    rule: &Rule,
    me: &PlayerReplayStats,
    others: &[&PlayerReplayStats],
    winners: &[PlayerReplayStats],
) -> Option<(f64, Insight)> {
    let v = (rule.value)(me).filter(|_| (rule.gate)(me))?;
    if !winners.is_empty() {
        let past: Vec<f64> = winners.iter().filter_map(|s| (rule.value)(s)).collect();
        let b = baseline(&past)?;
        let z = (v - b.mean) / b.sd;
        let rel_ok = b.mean <= 0.0 || (v - b.mean) / b.mean >= MIN_REL_WINNERS;
        let fact = format!(
            "{}: {} — winners average {}.",
            rule.label,
            show(rule.unit, v),
            show(rule.unit, b.mean)
        );
        return (z >= MIN_Z && rel_ok)
            .then(|| (z.min(MAX_SCORE), insight(rule.stat, fact, rule.tip)));
    }
    let vals: Vec<f64> = others.iter().filter_map(|o| (rule.value)(o)).collect();
    let max = vals.iter().copied().reduce(f64::max)?;
    let min = vals.iter().copied().reduce(f64::min)?;
    let mean = vals.iter().sum::<f64>() / vals.len() as f64;
    let score = if mean > 0.0 {
        (v / mean - 1.0) / MIN_REL_MATCH
    } else {
        // Over a zero mean any gap is "infinitely" worse; rank it at the threshold, not the top.
        1.0
    };
    if v <= max || score < 1.0 {
        return None;
    }
    let range = if vals.len() == 1 {
        format!("other {}", show(rule.unit, min))
    } else {
        format!("others {}–{}", show(rule.unit, min), show(rule.unit, max))
    };
    let fact = format!(
        "{}: {} — the most in this match ({range}).",
        rule.label,
        show(rule.unit, v)
    );
    Some((score.min(MAX_SCORE), insight(rule.stat, fact, rule.tip)))
}

/// The rules that always compare within the match (spec §3.2 rules 4, 6, 7 and 10).
fn match_only(me: &ReplayPlayer, players: &[ReplayPlayer], team_size: i64) -> Vec<(f64, Insight)> {
    let s = &me.stats;
    let mut out = Vec::new();
    if let (Some(w), Some(l)) = (s.fifty_wins, s.fifty_losses) {
        if l >= w + 3.0 {
            out.push((
                (l - w) / 3.0,
                insight(
                    "fifty_losses",
                    format!("50/50s: won {w}, lost {l}."),
                    "Challenge from the side the ball is leaving, and aim for the corner.",
                ),
            ));
        }
    }
    if let (Some(w), Some(l)) = (s.kickoff_wins, s.kickoff_losses) {
        if l >= w + 2.0 && l >= 3.0 {
            out.push(((l - w) / 2.0, insight("kickoff_losses", format!("Kickoffs: won {w}, lost {l}."),
                "Settle on one kickoff that hits the ball dead centre before working on speed flips.")));
        }
    }
    let share = |x: &PlayerReplayStats| {
        Some(x.secs_first_man? * 100.0 / x.tracked_seconds.filter(|t| *t > 0.0)?)
    };
    let mate = players
        .iter()
        .find(|q| q.team == me.team && !q.is_me && !q.is_bot);
    if let (2, Some(mine), Some(theirs)) = (team_size, share(s), mate.and_then(|q| share(&q.stats)))
    {
        if mine - theirs >= 15.0 {
            out.push((
                ((mine - theirs) / 15.0).min(MAX_SCORE),
                insight(
                    "secs_first_man",
                    format!(
                        "1st man: {}% of the match, your teammate {}%.",
                        mine.round(),
                        theirs.round()
                    ),
                    "When your teammate is closer and facing the ball, let them have it and cover.",
                ),
            ));
        }
    }
    let opponent = players.iter().find(|q| q.team != me.team && !q.is_bot);
    if let (1, Some(mine), Some(theirs)) = (
        team_size,
        s.boost_stolen,
        opponent.and_then(|q| q.stats.boost_stolen),
    ) {
        if theirs >= 300.0 && theirs >= 1.5 * mine {
            let score = if mine > 0.0 {
                theirs / mine / 1.5
            } else {
                MAX_SCORE
            };
            out.push((
                score.min(MAX_SCORE),
                insight(
                    "boost_stolen",
                    format!(
                        "Boost stolen: {} by your opponent, {} by you.",
                        theirs.round(),
                        mine.round()
                    ),
                    "After a shot, take their corner boost if your net is safe.",
                ),
            ));
        }
    }
    out
}

/// Up to `MAX_INSIGHTS`, highest score first (spec §3).
pub fn insights(
    players: &[ReplayPlayer],
    winners: &[PlayerReplayStats],
    team_size: i64,
) -> Vec<Insight> {
    let Some(me) = players.iter().find(|q| q.is_me) else {
        return Vec::new();
    };
    let others: Vec<&PlayerReplayStats> = players
        .iter()
        .filter(|q| !q.is_me && !q.is_bot)
        .map(|q| &q.stats)
        .collect();
    let mut found: Vec<(f64, Insight)> = COMPARED
        .iter()
        .filter_map(|r| compared(r, &me.stats, &others, winners))
        .collect();
    found.extend(match_only(me, players, team_size));
    found.sort_by(|a, b| b.0.total_cmp(&a.0));
    found
        .into_iter()
        .take(MAX_INSIGHTS)
        .map(|(_, i)| i)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::store::replays::ReplayPlayer;

    fn p(slot: i64, team: i64, me: bool, stats: PlayerReplayStats) -> ReplayPlayer {
        ReplayPlayer {
            slot,
            team,
            name: format!("P{slot}"),
            is_me: me,
            is_bot: false,
            stats,
        }
    }
    fn s(f: impl FnOnce(&mut PlayerReplayStats)) -> PlayerReplayStats {
        let mut x = PlayerReplayStats {
            tracked_seconds: Some(300.0),
            ..PlayerReplayStats::default()
        };
        f(&mut x);
        x
    }
    fn stats_of(found: &[Insight]) -> Vec<&str> {
        found.iter().map(|i| i.stat.as_str()).collect()
    }

    #[test]
    fn against_the_match_the_worst_by_30_percent_fires() {
        let one_v_one = |mine: f64| {
            insights(
                &[
                    p(0, 0, true, s(|x| x.pct_zero_boost = Some(mine))),
                    p(1, 1, false, s(|x| x.pct_zero_boost = Some(20.0))),
                ],
                &[],
                1,
            )
        };
        let found = one_v_one(26.0);
        assert_eq!(stats_of(&found), ["pct_zero_boost"]);
        assert_eq!(
            found[0].fact,
            "Time at 0 boost: 26% — the most in this match (other 20%)."
        );
        assert!(found[0].tip.starts_with("Route back over the small pads"));
        assert!(one_v_one(25.9).is_empty(), "under 30% worse");
    }

    #[test]
    fn against_the_winners_when_there_are_some() {
        let winners: Vec<PlayerReplayStats> = (0..10)
            .map(|i| s(|x| x.pct_zero_boost = Some(10.0 + (i % 2) as f64 * 4.0)))
            .collect();
        // mean 12, sd 2: 14 is 1 sd and 17% worse (no), 15 is 1.5 sd and 25% worse (yes)
        let me = |v: f64| {
            [
                p(0, 0, true, s(|x| x.pct_zero_boost = Some(v))),
                p(1, 1, false, s(|x| x.pct_zero_boost = Some(90.0))),
            ]
        };
        assert!(
            insights(&me(14.0), &winners, 1).is_empty(),
            "the opponent is ignored once there are winners"
        );
        let found = insights(&me(15.0), &winners, 1);
        assert_eq!(found[0].fact, "Time at 0 boost: 15% — winners average 12%.");
    }

    #[test]
    fn per_five_minutes_makes_long_matches_fair() {
        let them = p(1, 1, false, s(|x| x.whiffs = Some(4.0)));
        let short = insights(
            &[p(0, 0, true, s(|x| x.whiffs = Some(6.0))), them.clone()],
            &[],
            1,
        );
        assert_eq!(stats_of(&short), ["whiffs"]);
        assert_eq!(
            short[0].fact,
            "Whiffs per 5 min: 6.0 — the most in this match (other 4.0)."
        );
        let long = insights(
            &[
                p(
                    0,
                    0,
                    true,
                    s(|x| {
                        x.whiffs = Some(6.0);
                        x.tracked_seconds = Some(600.0)
                    }),
                ),
                them,
            ],
            &[],
            1,
        );
        assert!(long.is_empty(), "3.0 per 5 min is better than 4.0");
    }

    #[test]
    fn fifties_and_kickoffs_compare_wins_and_losses() {
        let me = s(|x| {
            x.fifty_wins = Some(2.0);
            x.fifty_losses = Some(5.0);
            x.kickoff_wins = Some(1.0);
            x.kickoff_losses = Some(3.0)
        });
        let found = insights(&[p(0, 0, true, me)], &[], 1);
        assert_eq!(stats_of(&found), ["fifty_losses", "kickoff_losses"]);
        assert_eq!(found[0].fact, "50/50s: won 2, lost 5.");
        assert_eq!(found[1].fact, "Kickoffs: won 1, lost 3.");
        let close = s(|x| {
            x.fifty_wins = Some(2.0);
            x.fifty_losses = Some(4.0);
            x.kickoff_wins = Some(1.0);
            x.kickoff_losses = Some(2.0)
        });
        assert!(insights(&[p(0, 0, true, close)], &[], 1).is_empty());
    }

    #[test]
    fn first_man_rule_needs_2v2() {
        let players = [
            p(0, 0, true, s(|x| x.secs_first_man = Some(183.0))),
            p(1, 0, false, s(|x| x.secs_first_man = Some(114.0))),
            p(2, 1, false, s(|_| {})),
            p(3, 1, false, s(|_| {})),
        ];
        let found = insights(&players, &[], 2);
        assert_eq!(
            found[0].fact,
            "1st man: 61% of the match, your teammate 38%."
        );
        assert!(insights(&players[..1], &[], 1).is_empty());
    }

    #[test]
    fn boost_stealing_rule_needs_1v1() {
        let players = [
            p(0, 0, true, s(|x| x.boost_stolen = Some(596.0))),
            p(1, 1, false, s(|x| x.boost_stolen = Some(1113.0))),
        ];
        let found = insights(&players, &[], 1);
        assert_eq!(stats_of(&found), ["boost_stolen"]);
        assert_eq!(
            found[0].fact,
            "Boost stolen: 1113 by your opponent, 596 by you."
        );
        assert!(insights(&players, &[], 2)
            .iter()
            .all(|i| i.stat != "boost_stolen"));
    }

    #[test]
    fn empty_players_never_fire() {
        let me = s(|x| {
            x.pct_zero_boost = Some(40.0);
            x.whiffs = Some(9.0)
        });
        let found = insights(
            &[
                p(0, 0, true, me),
                p(1, 1, false, PlayerReplayStats::default()),
            ],
            &[],
            1,
        );
        assert!(found.is_empty(), "nobody to compare with");
    }

    #[test]
    fn a_zero_mean_gives_a_capped_score() {
        let me = s(|x| x.demos_taken = Some(3.0));
        let found = insights(
            &[
                p(0, 0, true, me),
                p(1, 1, false, s(|x| x.demos_taken = Some(0.0))),
            ],
            &[],
            1,
        );
        assert_eq!(stats_of(&found), ["demos_taken"]);
        let once = s(|x| x.demos_taken = Some(1.0));
        assert!(
            insights(
                &[
                    p(0, 0, true, once),
                    p(1, 1, false, s(|x| x.demos_taken = Some(0.0)))
                ],
                &[],
                1
            )
            .is_empty(),
            "needs 2 demos"
        );
    }

    #[test]
    fn a_gap_over_a_zero_mean_ranks_below_a_real_gap() {
        let me = s(|x| {
            x.whiffs = Some(1.0);
            x.fifty_wins = Some(2.0);
            x.fifty_losses = Some(8.0); // score 2
        });
        let found = insights(
            &[
                p(0, 0, true, me),
                p(1, 1, false, s(|x| x.whiffs = Some(0.0))),
            ],
            &[],
            1,
        );
        assert_eq!(stats_of(&found), ["fifty_losses", "whiffs"]);
    }

    #[test]
    fn at_most_three_highest_score_first() {
        let me = s(|x| {
            x.pct_zero_boost = Some(60.0); // score capped at 3
            x.whiffs = Some(5.2); // 30% worse than 4: score 1
            x.fifty_wins = Some(0.0);
            x.fifty_losses = Some(6.0); // score 2
            x.kickoff_wins = Some(0.0);
            x.kickoff_losses = Some(5.0); // score 2.5
        });
        let them = s(|x| {
            x.pct_zero_boost = Some(10.0);
            x.whiffs = Some(4.0)
        });
        let found = insights(&[p(0, 0, true, me), p(1, 1, false, them)], &[], 1);
        assert_eq!(
            stats_of(&found),
            ["pct_zero_boost", "kickoff_losses", "fifty_losses"]
        );
    }
}
