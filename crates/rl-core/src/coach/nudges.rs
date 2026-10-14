//! Training nudges (spec §3.1): rate dropping and miss streak, rebuilt from the block's attempts
//! each time, so a restart neither loses nor repeats one.

use serde::Serialize;
use ts_rs::TS;

use super::rules::{counted, window_makes, Try, WINDOW};

/// The rate nudge fires this far below the block's best window…
pub const DROP_POINTS: f64 = 0.30;
/// …and re-arms once the last window is back within this of the best.
pub const REARM_POINTS: f64 = 0.15;
/// Misses in a row that fire the streak nudge.
pub const MISS_STREAK: u32 = 10;
const EPS: f64 = 1e-9;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Nudge {
    RateDrop {
        fired_at_attempt: i64,
        last20_rate: f64,
        best20_rate: f64,
    },
    MissStreak {
        fired_at_attempt: i64,
        count: u32,
    },
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct NudgeState {
    /// The nudge to show now, if any (the newer one when both are on).
    pub active: Option<Nudge>,
    /// Attempt ids where the rate nudge fired, in order.
    pub rate_drops: Vec<i64>,
    /// Attempt ids where the streak nudge fired, in order.
    pub streaks: Vec<i64>,
}

pub fn evaluate(tries: &[Try], goalie: bool) -> NudgeState {
    let c = counted(tries, goalie);
    let (rate_drops, rate_active) = rate_nudge(&c);
    let (streaks, streak_active) = streak_nudge(&c);
    let active = match (rate_active, streak_active) {
        (Some(r), Some(s)) => Some(if fired_at(&s) > fired_at(&r) { s } else { r }),
        (r, s) => r.or(s),
    };
    NudgeState {
        active,
        rate_drops,
        streaks,
    }
}

fn fired_at(n: &Nudge) -> i64 {
    match n {
        Nudge::RateDrop {
            fired_at_attempt, ..
        }
        | Nudge::MissStreak {
            fired_at_attempt, ..
        } => *fired_at_attempt,
    }
}

fn rate_nudge(c: &[Try]) -> (Vec<i64>, Option<Nudge>) {
    let mut fired = Vec::new();
    let mut best = 0.0_f64;
    let mut open: Option<i64> = None;
    let mut last = 0.0;
    for (i, makes) in window_makes(c).into_iter().enumerate() {
        let rate = f64::from(makes) / WINDOW as f64;
        best = best.max(rate);
        last = rate;
        let id = c[i + WINDOW - 1].id;
        match open {
            None if best - rate >= DROP_POINTS - EPS => {
                fired.push(id);
                open = Some(id);
            }
            Some(_) if best - rate <= REARM_POINTS + EPS => open = None,
            _ => {}
        }
    }
    let active = open.map(|id| Nudge::RateDrop {
        fired_at_attempt: id,
        last20_rate: last,
        best20_rate: best,
    });
    (fired, active)
}

fn streak_nudge(c: &[Try]) -> (Vec<i64>, Option<Nudge>) {
    let mut fired = Vec::new();
    let mut run = 0_u32;
    let mut open: Option<i64> = None;
    for t in c {
        if t.made {
            run = 0;
            open = None;
            continue;
        }
        run += 1;
        if run == MISS_STREAK && open.is_none() {
            fired.push(t.id);
            open = Some(t.id);
        }
    }
    let active = open.map(|id| Nudge::MissStreak {
        fired_at_attempt: id,
        count: run,
    });
    (fired, active)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tries(spec: &str) -> Vec<Try> {
        spec.split_whitespace()
            .enumerate()
            .map(|(i, tok)| Try {
                id: i as i64 + 1,
                started_at: i as i64 * 20_000,
                made: tok.starts_with('M'),
                duration_ms: tok[1..].parse().unwrap(),
                skipped: false,
            })
            .collect()
    }

    /// `n` attempts of 4 s: `M` or `X` repeated from `pattern`.
    fn run(pattern: &str, n: usize) -> String {
        pattern
            .chars()
            .cycle()
            .take(n)
            .map(|c| format!("{c}4000 "))
            .collect()
    }

    fn fixture(label: &str) -> Vec<Try> {
        let text = include_str!("../../tests/fixtures/nudge-sequences.txt");
        let line = text.lines().find(|l| l.starts_with(label)).unwrap();
        tries(line.split('|').nth(1).unwrap())
    }

    /// Positions (1-based, among counted attempts) where each nudge fired.
    fn positions(tries: &[Try], ids: &[i64]) -> Vec<usize> {
        let c = counted(tries, false);
        ids.iter()
            .map(|id| c.iter().position(|t| t.id == *id).unwrap() + 1)
            .collect()
    }

    /// 12 makes then 8 misses: the first window is 60%.
    fn base() -> String {
        run("M", 12) + &run("X", 8)
    }

    #[test]
    fn the_rate_nudge_fires_at_thirty_points_below_the_best_window() {
        let t = tries(&(base() + &run("X", 6)));
        let s = evaluate(&t, false);
        assert_eq!(positions(&t, &s.rate_drops), [26]);
        match s.active {
            Some(Nudge::RateDrop {
                last20_rate,
                best20_rate,
                ..
            }) => {
                assert!((best20_rate - 0.6).abs() < 1e-9);
                assert!((last20_rate - 0.3).abs() < 1e-9);
            }
            other => panic!("expected the newer rate drop, got {other:?}"),
        }
        let t5 = tries(&(base() + &run("X", 5)));
        assert!(
            evaluate(&t5, false).rate_drops.is_empty(),
            "35% is only 25 points below"
        );
    }

    #[test]
    fn the_rate_nudge_rearms_within_fifteen_points() {
        let t = tries(&(base() + &run("X", 6) + &run("M", 9)));
        assert!(
            evaluate(&t, false).active.is_none(),
            "back to 45%, within 15 points of 60%"
        );
        let again = tries(&(base() + &run("X", 6) + &run("M", 9) + &run("X", 14)));
        assert_eq!(
            positions(&again, &evaluate(&again, false).rate_drops),
            [26, 49]
        );
    }

    #[test]
    fn the_streak_nudge_fires_at_ten_and_counts_on() {
        let t = tries(&(run("M", 3) + &run("X", 14)));
        let s = evaluate(&t, false);
        assert_eq!(positions(&t, &s.streaks), [13]);
        assert!(matches!(
            s.active,
            Some(Nudge::MissStreak { count: 14, .. })
        ));
        let made = tries(&(run("M", 3) + &run("X", 14) + "M4000"));
        assert!(
            evaluate(&made, false).active.is_none(),
            "a make re-arms and clears it"
        );
    }

    #[test]
    fn quick_resets_neither_break_nor_extend_a_streak() {
        let t = tries(&(run("X", 9) + "X1200 X900 " + "X4000"));
        assert_eq!(
            evaluate(&t, false).streaks.len(),
            1,
            "the 10th counted miss fires"
        );
    }

    #[test]
    fn the_same_attempts_give_the_same_nudge() {
        let t = fixture("2026-10-12");
        assert_eq!(evaluate(&t, false), evaluate(&t, false));
    }

    #[test]
    fn the_users_sessions_fire_where_expected() {
        let cases: [(&str, &[usize], &[usize]); 5] = [
            ("2026-10-08-evening-a", &[], &[]),
            ("2026-10-08-evening-b", &[], &[20, 45]),
            ("2026-10-08-night", &[39], &[]),
            ("2026-10-09", &[62, 107], &[]),
            (
                "2026-10-12",
                &[27, 71, 127, 152],
                &[26, 61, 84, 96, 127, 152],
            ),
        ];
        for (label, rate, streak) in cases {
            let t = fixture(label);
            let s = evaluate(&t, false);
            assert_eq!(positions(&t, &s.rate_drops), rate, "{label} rate");
            assert_eq!(positions(&t, &s.streaks), streak, "{label} streak");
        }
    }
}
