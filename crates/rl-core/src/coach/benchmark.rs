//! What wins at my level: the players of my recent ranked replays in a mode, winners against
//! losers (winners' benchmark spec §2). Pure: pool rows in, the benchmark out.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};
use ts_rs::TS;

use super::insights::{FIVE_MIN, PER_FIVE};
use crate::store::replays::{PlayerReplayStats, PoolRow};

/// Pool matches before the benchmark exists.
pub const MIN_POOL_MATCHES: usize = 10;
/// A standardised winners/losers difference this large "separates" (a small but real effect).
pub const MIN_GAP: f64 = 0.3;
/// Values needed on each side before a stat gets a gap.
const MIN_SIDE: usize = 5;
/// The result itself, not how to get it (spec §2.2).
const OUTCOMES: [&str; 6] = [
    "tracked_seconds",
    "score",
    "goals",
    "assists",
    "saves",
    "shots",
];

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct BenchStat {
    /// (winners − losers) / pooled sd: positive means more is better.
    pub gap: f64,
    pub separates: bool,
    /// My percentile in the pool, flipped when less is better, so higher is always better.
    pub me: Option<f64>,
    /// The winners' mean percentile, on the same scale.
    pub winners_pct: f64,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, TS)]
pub struct Benchmark {
    /// Matches in the pool (the page says how many more are needed).
    pub matches: i64,
    /// Winners' means, each row scaled to 5 minutes first; `None` under `MIN_POOL_MATCHES`.
    pub winners: Option<PlayerReplayStats>,
    pub losers: Option<PlayerReplayStats>,
    /// Per stat by field name; empty under `MIN_POOL_MATCHES`.
    pub stats: BTreeMap<String, BenchStat>,
}

fn timed(s: &PlayerReplayStats) -> Option<f64> {
    s.tracked_seconds.filter(|t| *t > 0.0)
}

fn fields(s: &PlayerReplayStats) -> Map<String, Value> {
    match serde_json::to_value(s) {
        Ok(Value::Object(m)) => m,
        _ => Map::new(),
    }
}

/// A stat as compared: counts per 5 minutes, low-boost touches as a share of touches, the rest as is.
fn value(key: &str, f: &Map<String, Value>, t: f64) -> Option<f64> {
    let x = f.get(key)?.as_f64()?;
    if key == "touches_low_boost" {
        let all = f.get("touches")?.as_f64().filter(|a| *a > 0.0)?;
        return Some(x * 100.0 / all);
    }
    Some(if PER_FIVE.contains(&key) {
        x * FIVE_MIN / t
    } else {
        x
    })
}

/// The mean per stat, each row first scaled to a 5-minute match, so the page's per-5-minute
/// values read the same on it. `None` without a timed row.
pub fn mean_per_five(rows: &[&PlayerReplayStats]) -> Option<PlayerReplayStats> {
    let mut sums: BTreeMap<String, (f64, f64)> = BTreeMap::new();
    for s in rows {
        let Some(t) = timed(s) else { continue };
        for (k, v) in fields(s) {
            let Some(mut x) = v.as_f64() else { continue };
            if PER_FIVE.contains(&k.as_str()) {
                x *= FIVE_MIN / t;
            }
            let e = sums.entry(k).or_default();
            e.0 += x;
            e.1 += 1.0;
        }
    }
    if sums.is_empty() {
        return None;
    }
    let means: Map<String, Value> = sums
        .into_iter()
        .map(|(k, (sum, n))| (k, Value::from(sum / n)))
        .collect();
    let mut m: PlayerReplayStats = serde_json::from_value(Value::Object(means)).ok()?;
    m.tracked_seconds = Some(FIVE_MIN);
    Some(m)
}

fn mean_var(xs: &[f64]) -> (f64, f64) {
    let n = xs.len() as f64;
    let m = xs.iter().sum::<f64>() / n;
    (
        m,
        xs.iter().map(|x| (x - m).powi(2)).sum::<f64>() / (n - 1.0),
    )
}

/// (winners − losers) / pooled sd; 0 when nothing varies.
fn gap(w: &[f64], l: &[f64]) -> f64 {
    let ((mw, vw), (ml, vl)) = (mean_var(w), mean_var(l));
    let (nw, nl) = (w.len() as f64, l.len() as f64);
    let sd = (((nw - 1.0) * vw + (nl - 1.0) * vl) / (nw + nl - 2.0)).sqrt();
    if sd > 1e-9 {
        (mw - ml) / sd
    } else {
        0.0
    }
}

/// Share of `all` below `v`, ties counting half, 0–100.
fn percentile(v: f64, all: &[f64]) -> f64 {
    let below = all.iter().filter(|x| **x < v).count() as f64;
    let ties = all.iter().filter(|x| **x == v).count() as f64;
    (below + ties / 2.0) * 100.0 / all.len() as f64
}

/// The winning players' rows the insights compare with; empty under `MIN_POOL_MATCHES`.
pub fn winner_rows(pool: &[PoolRow], matches: usize) -> Vec<PlayerReplayStats> {
    if matches < MIN_POOL_MATCHES {
        return Vec::new();
    }
    pool.iter()
        .filter(|r| r.won && timed(&r.stats).is_some())
        .map(|r| r.stats.clone())
        .collect()
}

/// The benchmark from a pool (spec §2.3); `me` are my rows: one on the match page, my pool rows
/// on Progress, where my percentile is their mean (progress radar spec §3).
pub fn benchmark(pool: &[PoolRow], matches: usize, me: &[&PlayerReplayStats]) -> Benchmark {
    if matches < MIN_POOL_MATCHES {
        return Benchmark {
            matches: matches as i64,
            ..Benchmark::default()
        };
    }
    let rows: Vec<(bool, f64, Map<String, Value>)> = pool
        .iter()
        .filter_map(|r| Some((r.won, timed(&r.stats)?, fields(&r.stats))))
        .collect();
    let mine: Vec<(f64, Map<String, Value>)> = me
        .iter()
        .filter_map(|s| Some((timed(s)?, fields(s))))
        .collect();
    let mut stats = BTreeMap::new();
    for key in fields(&PlayerReplayStats::default()).keys() {
        if OUTCOMES.contains(&key.as_str()) {
            continue;
        }
        let side = |won: bool| -> Vec<f64> {
            rows.iter()
                .filter(|r| r.0 == won)
                .filter_map(|r| value(key, &r.2, r.1))
                .collect()
        };
        let (w, l) = (side(true), side(false));
        if w.len() < MIN_SIDE || l.len() < MIN_SIDE {
            continue;
        }
        let g = gap(&w, &l);
        let all: Vec<f64> = w.iter().chain(&l).copied().collect();
        let oriented = |v: f64| {
            if g < 0.0 {
                100.0 - percentile(v, &all)
            } else {
                percentile(v, &all)
            }
        };
        stats.insert(
            key.clone(),
            BenchStat {
                gap: g,
                separates: g.abs() >= MIN_GAP,
                me: {
                    let pcts: Vec<f64> = mine
                        .iter()
                        .filter_map(|(t, f)| value(key, f, *t))
                        .map(oriented)
                        .collect();
                    (!pcts.is_empty()).then(|| pcts.iter().sum::<f64>() / pcts.len() as f64)
                },
                winners_pct: w.iter().map(|v| oriented(*v)).sum::<f64>() / w.len() as f64,
            },
        );
    }
    let means = |won: bool| {
        mean_per_five(
            &pool
                .iter()
                .filter(|r| r.won == won)
                .map(|r| &r.stats)
                .collect::<Vec<_>>(),
        )
    };
    Benchmark {
        matches: matches as i64,
        winners: means(true),
        losers: means(false),
        stats,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(won: bool, whiffs: f64) -> PoolRow {
        PoolRow {
            is_me: false,
            won,
            stats: PlayerReplayStats {
                tracked_seconds: Some(300.0),
                whiffs: Some(whiffs),
                pct_zero_boost: Some(if won { 10.0 } else { 14.0 }),
                kickoff_wins: Some(1.0),
                goals: Some(if won { 3.0 } else { 0.0 }),
                ..PlayerReplayStats::default()
            },
        }
    }
    /// 10 matches: winners whiff 1, 2, 3, 1, 2, 3, 1, 2, 3, 1; losers 4, 5, 6, …
    fn pool() -> Vec<PoolRow> {
        (0..10)
            .flat_map(|i| {
                let k = (i % 3) as f64;
                [r(true, 1.0 + k), r(false, 4.0 + k)]
            })
            .collect()
    }
    fn me(t: f64, whiffs: f64) -> PlayerReplayStats {
        PlayerReplayStats {
            tracked_seconds: Some(t),
            whiffs: Some(whiffs),
            ..PlayerReplayStats::default()
        }
    }

    #[test]
    fn no_benchmark_under_ten_matches() {
        let b = benchmark(&pool(), 9, &[]);
        assert_eq!((b.matches, b.winners, b.losers), (9, None, None));
        assert!(b.stats.is_empty());
        assert!(winner_rows(&pool(), 9).is_empty());
        assert_eq!(winner_rows(&pool(), 10).len(), 10);
    }

    #[test]
    fn less_is_better_flips_my_percentile() {
        let b = benchmark(&pool(), 10, &[&me(300.0, 1.0)]);
        let w = &b.stats["whiffs"];
        assert!(w.gap < -MIN_GAP && w.separates);
        // 1 whiff: none below, 4 ties of 20 values → 10th percentile, flipped to 90
        assert_eq!(w.me, Some(90.0));
        assert!(w.winners_pct > 50.0);
        let long = benchmark(&pool(), 10, &[&me(600.0, 2.0)]);
        assert_eq!(
            long.stats["whiffs"].me,
            Some(90.0),
            "2 in 10 min is 1 per 5 min"
        );
    }

    #[test]
    fn means_are_per_five_minutes() {
        let b = benchmark(&pool(), 10, &[]);
        assert_eq!(b.winners.unwrap().whiffs, Some(1.9));
        assert_eq!(b.losers.unwrap().whiffs, Some(4.9));
    }

    #[test]
    fn outcomes_are_left_out_and_small_gaps_do_not_separate() {
        let mut p = pool();
        p.iter_mut()
            .filter(|r| !r.won)
            .take(3)
            .for_each(|r| r.stats.pct_zero_boost = Some(10.0));
        let b = benchmark(&p, 10, &[]);
        assert!(!b.stats.contains_key("goals") && !b.stats.contains_key("tracked_seconds"));
        assert!(b.stats["pct_zero_boost"].separates, "10 vs mostly 14");
        // the same values on both sides of each match: no gap
        let even: Vec<PoolRow> = pool()
            .into_iter()
            .enumerate()
            .map(|(j, mut r)| {
                r.stats.pct_zero_boost = Some(12.0 + ((j / 2) % 2) as f64);
                r
            })
            .collect();
        assert!(!benchmark(&even, 10, &[]).stats["pct_zero_boost"].separates);
    }

    #[test]
    fn no_spread_gives_a_zero_gap() {
        let k = &benchmark(&pool(), 10, &[]).stats["kickoff_wins"];
        assert_eq!((k.gap, k.separates), (0.0, false));
    }

    #[test]
    fn my_percentile_is_the_mean_over_my_rows() {
        // 1 whiff -> 90 (see less_is_better_flips_my_percentile); 4 whiffs: 10 below, 4 ties of
        // 20 -> 60th, flipped to 40. Mean 65. The untimed row is left out.
        let untimed = PlayerReplayStats {
            whiffs: Some(9.0),
            ..PlayerReplayStats::default()
        };
        let b = benchmark(&pool(), 10, &[&me(300.0, 1.0), &me(300.0, 4.0), &untimed]);
        assert_eq!(b.stats["whiffs"].me, Some(65.0));
        assert_eq!(b.stats["pct_zero_boost"].me, None, "none of my rows has it");
    }

    #[test]
    fn missing_values_are_skipped() {
        let mut p = pool();
        p.push(PoolRow {
            is_me: false,
            won: true,
            stats: PlayerReplayStats {
                whiffs: Some(99.0),
                ..PlayerReplayStats::default()
            },
        });
        p.iter_mut()
            .take(4)
            .for_each(|r| r.stats.kickoff_wins = None);
        let b = benchmark(&p, 10, &[&me(300.0, 1.0)]);
        assert_eq!(
            b.winners.unwrap().whiffs,
            Some(1.9),
            "the untimed row is out"
        );
        assert_eq!(b.stats["pct_zero_boost"].me, None, "my row has no value");
        assert!(
            b.stats.contains_key("kickoff_wins"),
            "8 winners and 8 losers still have it"
        );
        assert!(!b.stats.contains_key("hard_hits"), "nobody has it");
    }

    #[test]
    fn low_boost_touches_are_a_share_of_touches() {
        // winners 10% or 15% of their 20 touches, losers 50% or 60% of their 10 (a spread, or the gap is 0)
        let p: Vec<PoolRow> = pool()
            .into_iter()
            .enumerate()
            .map(|(j, mut r)| {
                let extra = (j / 2 % 2) as f64;
                (r.stats.touches, r.stats.touches_low_boost) = if r.won {
                    (Some(20.0), Some(2.0 + extra))
                } else {
                    (Some(10.0), Some(5.0 + extra))
                };
                r
            })
            .collect();
        let mine = PlayerReplayStats {
            touches: Some(10.0),
            touches_low_boost: Some(1.0),
            ..me(300.0, 1.0)
        };
        // 10%: 5 ties of 20 values → 12.5th percentile, flipped (less is better) to 87.5
        assert_eq!(
            benchmark(&p, 10, &[&mine]).stats["touches_low_boost"].me,
            Some(87.5)
        );
    }
}
