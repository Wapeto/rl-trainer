//! The after-session review (spec §5.1), built from what the store read.

use serde::Serialize;
use ts_rs::TS;

use super::nudges::evaluate;
use super::rules::{best_stretch, counted, Try};
use super::usual::{
    baseline, mode_stat_meta, replay_meta, replay_value, Better, ReplayStat, ReviewUnit,
};
use crate::store::progress::rules::{mean, Weighted};
use crate::store::progress::stats::{sample, MatchRow};
use crate::store::progress::ModeStat;
use crate::store::replays::{ReplayStats, ReplayToSave};

pub const MIN_EARLIER: usize = 5;
pub const MIN_EARLIER_REPLAYS: usize = 3;
pub const MIN_SCORE: f64 = 1.0;
pub const MAX_ITEMS: usize = 3;
pub const BAR_SIZE: usize = 10;

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct SessionReview {
    pub session_id: i64,
    pub training: Vec<TrainingItem>,
    pub modes: Vec<ModeReview>,
    pub to_save: Vec<ReplayToSave>,
    /// When a replay of this session arrived after it ended (the review changed).
    pub replay_added_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct TrainingItem {
    pub pack_id: Option<i64>,
    pub pack_name: Option<String>,
    /// The first attempt on the pack this session.
    pub started_at: i64,
    pub makes: i64,
    pub counted: i64,
    pub quick_resets: i64,
    pub make_rate: Option<f64>,
    pub target_rate: f64,
    /// Makes in the best 20-attempt stretch, and when it ended.
    pub best_makes: Option<u32>,
    pub best_at: Option<i64>,
    /// When the rate nudge first fired.
    pub dropped_at: Option<i64>,
    /// Make rate per 10 counted attempts, oldest first.
    pub bars: Vec<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ModeReview {
    pub team_size: i64,
    pub matches: i64,
    /// Earlier ranked matches of this mode used as "usual" (up to 20).
    pub earlier: i64,
    pub items: Vec<ReviewItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, TS)]
pub struct ReviewItem {
    pub key: String,
    pub label: String,
    pub unit: ReviewUnit,
    pub value: f64,
    pub usual: f64,
    pub better: bool,
    /// How widely the stat usually varies (the standard deviation of the earlier matches).
    pub spread: f64,
    /// Matches of the session the value covers, out of `of` (replay stats: those with a replay).
    pub covered: i64,
    pub of: i64,
}

pub struct TrainingInput {
    pub pack_id: Option<i64>,
    pub pack_name: Option<String>,
    pub goalie: bool,
    pub target_rate: f64,
    pub tries: Vec<Try>,
}

pub type MatchInput = (MatchRow, Option<ReplayStats>);

pub struct ModeInput {
    pub team_size: i64,
    pub session: Vec<MatchInput>,
    pub earlier: Vec<MatchInput>,
}

pub fn build(
    session_id: i64,
    training: &[TrainingInput],
    modes: &[ModeInput],
    to_save: Vec<ReplayToSave>,
    replay_added_at: Option<i64>,
) -> SessionReview {
    SessionReview {
        session_id,
        training: training_items(training),
        modes: modes.iter().map(mode_review).collect(),
        to_save,
        replay_added_at,
    }
}

fn training_items(blocks: &[TrainingInput]) -> Vec<TrainingItem> {
    let mut packs: Vec<Option<i64>> = Vec::new();
    for b in blocks {
        if !packs.contains(&b.pack_id) {
            packs.push(b.pack_id);
        }
    }
    packs
        .into_iter()
        .filter_map(|pack| {
            let mine: Vec<&TrainingInput> = blocks.iter().filter(|b| b.pack_id == pack).collect();
            training_item(&mine)
        })
        .collect()
}

fn training_item(blocks: &[&TrainingInput]) -> Option<TrainingItem> {
    let first = blocks.first()?;
    let all: Vec<Try> = blocks
        .iter()
        .flat_map(|b| b.tries.iter().copied())
        .collect();
    let started_at = all.first()?.started_at;
    let c = counted(&all, first.goalie);
    let makes = c.iter().filter(|t| t.made).count() as i64;
    let quick_resets = all.iter().filter(|t| !t.skipped).count() as i64 - c.len() as i64;
    let best = best_stretch(&c);
    let dropped_at = blocks
        .iter()
        .filter_map(|b| evaluate(&b.tries, b.goalie).rate_drops.first().copied())
        .filter_map(|id| all.iter().find(|t| t.id == id).map(|t| t.started_at))
        .min();
    Some(TrainingItem {
        pack_id: first.pack_id,
        pack_name: first.pack_name.clone(),
        started_at,
        makes,
        counted: c.len() as i64,
        quick_resets,
        make_rate: (!c.is_empty()).then(|| makes as f64 / c.len() as f64),
        target_rate: first.target_rate,
        best_makes: best.map(|b| b.makes),
        best_at: best.map(|b| c[b.end].started_at),
        dropped_at,
        bars: c
            .chunks(BAR_SIZE)
            .map(|ch| ch.iter().filter(|t| t.made).count() as f64 / ch.len() as f64)
            .collect(),
    })
}

struct Candidate {
    item: ReviewItem,
    score: f64,
}

fn mode_review(m: &ModeInput) -> ModeReview {
    let mut items: Vec<Candidate> = Vec::new();
    if m.earlier.len() >= MIN_EARLIER {
        items.extend(ModeStat::ALL.iter().filter_map(|s| mode_candidate(*s, m)));
        items.extend(
            ReplayStat::ALL
                .iter()
                .filter_map(|s| replay_candidate(*s, m)),
        );
    }
    items.sort_by(|a, b| b.score.abs().total_cmp(&a.score.abs()));
    ModeReview {
        team_size: m.team_size,
        matches: m.session.len() as i64,
        earlier: m.earlier.len() as i64,
        items: items.into_iter().take(MAX_ITEMS).map(|c| c.item).collect(),
    }
}

fn judged(
    key: &str,
    (label, unit, better): (&'static str, ReviewUnit, Better),
    value: f64,
    usual: &[f64],
    covered: i64,
    of: i64,
) -> Option<Candidate> {
    if better == Better::Neither {
        return None;
    }
    let b = baseline(usual)?;
    let score = (value - b.mean) / b.sd;
    if score.abs() < MIN_SCORE {
        return None;
    }
    Some(Candidate {
        item: ReviewItem {
            key: key.to_owned(),
            label: label.to_owned(),
            unit,
            value,
            usual: b.mean,
            better: (score > 0.0) == (better == Better::Higher),
            spread: b.sd,
            covered,
            of,
        },
        score,
    })
}

fn mode_candidate(stat: ModeStat, m: &ModeInput) -> Option<Candidate> {
    let now: Vec<Weighted> = m
        .session
        .iter()
        .filter_map(|(r, _)| sample(stat, r))
        .collect();
    let value = mean(&now)?;
    let usual: Vec<f64> = m
        .earlier
        .iter()
        .filter_map(|(r, _)| sample(stat, r))
        .map(|w| w.value)
        .collect();
    let key = serde_json::to_value(stat).ok()?.as_str()?.to_owned();
    let n = m.session.len() as i64;
    judged(&key, mode_stat_meta(stat), value, &usual, n, n)
}

fn replay_candidate(stat: ReplayStat, m: &ModeInput) -> Option<Candidate> {
    let now: Vec<f64> = m
        .session
        .iter()
        .filter_map(|(_, r)| replay_value(stat, r.as_ref()?))
        .collect();
    let usual: Vec<f64> = m
        .earlier
        .iter()
        .filter_map(|(_, r)| replay_value(stat, r.as_ref()?))
        .collect();
    if now.is_empty() || usual.len() < MIN_EARLIER_REPLAYS {
        return None;
    }
    let value = now.iter().sum::<f64>() / now.len() as f64;
    judged(
        stat.key(),
        replay_meta(stat),
        value,
        &usual,
        now.len() as i64,
        m.session.len() as i64,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coach::rules::Try;

    fn tries(spec: &str) -> Vec<Try> {
        spec.chars()
            .enumerate()
            .map(|(i, c)| Try {
                id: i as i64 + 1,
                started_at: 1_000_000 + i as i64 * 20_000,
                made: c == 'M',
                duration_ms: if c == 'q' { 1_000 } else { 4_000 },
                skipped: false,
            })
            .collect()
    }

    fn row(id: i64, goals: f64) -> MatchRow {
        MatchRow {
            match_id: id,
            team_size: 2,
            result: "win".into(),
            goals,
            shots: 4.0,
            ..MatchRow::default()
        }
    }

    fn training(spec: &str) -> TrainingInput {
        TrainingInput {
            pack_id: Some(11),
            pack_name: Some("Aerial Shots - Pass".into()),
            goalie: false,
            target_rate: 0.65,
            tries: tries(spec),
        }
    }

    #[test]
    fn the_training_item_leaves_out_quick_resets_and_finds_the_drop() {
        let spec = "MMMMMMMMMMMMXXXXXXXX".to_owned() + "qq" + "XXXXXX";
        let r = build(1, &[training(&spec)], &[], vec![], None);
        let t = &r.training[0];
        assert_eq!((t.makes, t.counted, t.quick_resets), (12, 26, 2));
        assert_eq!(t.best_makes, Some(12));
        assert!(t.dropped_at.is_some(), "the rate nudge fired in this block");
        assert_eq!(t.bars.len(), 3, "26 counted attempts in bars of 10");
        assert!((t.bars[0] - 1.0).abs() < 1e-9);
    }

    #[test]
    fn two_blocks_of_one_pack_make_one_item() {
        let r = build(1, &[training("MMX"), training("XXM")], &[], vec![], None);
        assert_eq!(r.training.len(), 1);
        assert_eq!((r.training[0].makes, r.training[0].counted), (3, 6));
    }

    #[test]
    fn fewer_than_five_earlier_matches_compare_nothing() {
        let mode = ModeInput {
            team_size: 1,
            session: vec![(row(10, 6.0), None)],
            earlier: (1..=3).map(|i| (row(i, 1.0 + i as f64), None)).collect(),
        };
        let r = build(1, &[], &[mode], vec![], None);
        assert_eq!(r.modes[0].earlier, 3);
        assert!(r.modes[0].items.is_empty());
    }

    #[test]
    fn the_stats_furthest_from_usual_are_shown_up_to_three() {
        let earlier: Vec<_> = (1..=10)
            .map(|i| (row(i, 1.0 + (i % 2) as f64), None))
            .collect();
        let mode = ModeInput {
            team_size: 2,
            session: vec![(row(20, 5.0), None)],
            earlier,
        };
        let r = build(1, &[], &[mode], vec![], None);
        let items = &r.modes[0].items;
        assert!(!items.is_empty() && items.len() <= 3);
        let goals = items
            .iter()
            .find(|i| i.key == "goals")
            .expect("goals stand out");
        assert!(goals.better);
        assert!((goals.usual - 1.5).abs() < 1e-9);
        // Earlier goals alternate 1 and 2: population standard deviation 0.5.
        assert!((goals.spread - 0.5).abs() < 1e-9, "spread {}", goals.spread);
        assert!(
            items.iter().all(|i| i.key != "win_rate"),
            "the outcome is never judged"
        );
    }

    #[test]
    fn nothing_stands_out_when_the_session_is_usual() {
        let earlier: Vec<_> = (1..=10)
            .map(|i| (row(i, 1.0 + (i % 2) as f64), None))
            .collect();
        let mode = ModeInput {
            team_size: 2,
            session: vec![(row(20, 1.5), None)],
            earlier,
        };
        assert!(build(1, &[], &[mode], vec![], None).modes[0]
            .items
            .is_empty());
    }

    #[test]
    fn replay_items_cover_only_matches_with_a_replay() {
        let rep = |low: i64| ReplayStats {
            touches: 40,
            touches_low_boost: low,
            ..ReplayStats::default()
        };
        let earlier: Vec<_> = (1..=6)
            .map(|i| (row(i, 1.5), Some(rep(8 + (i % 2) * 4))))
            .collect();
        let mode = ModeInput {
            team_size: 2,
            session: vec![(row(20, 1.5), Some(rep(30))), (row(21, 1.5), None)],
            earlier,
        };
        let r = build(1, &[], &[mode], vec![], None);
        let low = r.modes[0]
            .items
            .iter()
            .find(|i| i.key == "low_boost_touches")
            .unwrap();
        assert!(!low.better);
        assert_eq!((low.covered, low.of), (1, 2));
    }

    #[test]
    fn casual_matches_get_no_mode_section() {
        let r = build(1, &[], &[], vec![], None);
        assert!(r.modes.is_empty() && r.training.is_empty());
    }
}
