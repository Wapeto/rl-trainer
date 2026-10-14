//! What counts as a try (spec §2) and the 20-attempt windows the nudges and the review use.

/// A miss shorter than this is a quick reset (a bad start), outside goalie packs.
pub const QUICK_RESET_MS: i64 = 2_500;
/// Attempts in one rolling window.
pub const WINDOW: usize = 20;

/// One stored training attempt, as the coach reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Try {
    pub id: i64,
    pub started_at: i64,
    pub made: bool,
    pub duration_ms: i64,
    pub skipped: bool,
}

/// Counts toward the make rate: not skipped, and not a quick reset. In a goalie pack a miss is a
/// goal let in, which can be fast, so every miss counts there.
pub fn is_counted(t: &Try, goalie: bool) -> bool {
    !t.skipped && (goalie || t.made || t.duration_ms >= QUICK_RESET_MS)
}

pub fn counted(tries: &[Try], goalie: bool) -> Vec<Try> {
    tries
        .iter()
        .filter(|t| is_counted(t, goalie))
        .copied()
        .collect()
}

/// Makes in each window of `WINDOW` counted attempts, one per window end, oldest first.
pub fn window_makes(counted: &[Try]) -> Vec<u32> {
    counted
        .windows(WINDOW)
        .map(|w| w.iter().filter(|t| t.made).count() as u32)
        .collect()
}

/// The best window so far: its makes and the index of its last attempt (the first such window).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Stretch {
    pub makes: u32,
    pub end: usize,
}

pub fn best_stretch(counted: &[Try]) -> Option<Stretch> {
    window_makes(counted)
        .into_iter()
        .enumerate()
        .fold(None, |best: Option<Stretch>, (i, makes)| match best {
            Some(b) if b.makes >= makes => Some(b),
            _ => Some(Stretch {
                makes,
                end: i + WINDOW - 1,
            }),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(id: i64, made: bool, ms: i64) -> Try {
        Try {
            id,
            started_at: id * 10_000,
            made,
            duration_ms: ms,
            skipped: false,
        }
    }

    #[test]
    fn a_miss_under_two_and_a_half_seconds_is_a_quick_reset() {
        assert!(!is_counted(&t(1, false, 2_499), false));
        assert!(is_counted(&t(2, false, 2_500), false));
        assert!(is_counted(&t(3, true, 900), false), "a make is never quick");
        let skipped = Try {
            skipped: true,
            ..t(4, true, 5_000)
        };
        assert!(!is_counted(&skipped, false));
    }

    #[test]
    fn a_fast_miss_in_a_goalie_pack_still_counts() {
        assert!(is_counted(&t(1, false, 800), true));
    }

    #[test]
    fn windows_and_the_best_stretch() {
        let tries: Vec<Try> = (0..25).map(|i| t(i, i < 8, 4_000)).collect();
        let w = window_makes(&tries);
        assert_eq!(w.len(), 6);
        assert_eq!(w[0], 8, "attempts 0..20 hold the 8 makes");
        assert_eq!(w[5], 3, "attempts 5..25");
        assert_eq!(best_stretch(&tries), Some(Stretch { makes: 8, end: 19 }));
        assert_eq!(best_stretch(&tries[..19]), None);
    }
}
