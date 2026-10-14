//! Custom-training attempts: `RoundStarted` opens an attempt; the next countdown or round closes it.
//! A goal during the attempt makes it a make; anything else that is kept is a miss.

use crate::events::GameState;
use crate::facts::{Attempt, AttemptOutcome};

/// Attempts without a goal that end sooner than this are reset artefacts, not real tries.
pub const MIN_ATTEMPT_MS: i64 = 1_000;

#[derive(Debug)]
struct OpenAttempt {
    started_at_ms: i64,
    max_time: Option<f64>,
    /// The timer dropped below a value seen earlier in this attempt.
    ticked: bool,
    goal: Option<(i64, f64)>,
    retry_after_make: bool,
}

impl OpenAttempt {
    fn new(started_at_ms: i64, retry_after_make: bool) -> Self {
        Self {
            started_at_ms,
            max_time: None,
            ticked: false,
            goal: None,
            retry_after_make,
        }
    }

    fn observe_time(&mut self, time: f64) {
        match self.max_time {
            // Before the countdown visibly started, a higher value means the earlier samples
            // were stale (the previous shot's timer): start over from this one.
            Some(max) if !self.ticked && time > max => self.max_time = Some(time),
            Some(max) => {
                self.ticked |= time < max;
                self.max_time = Some(max.max(time));
            }
            None => self.max_time = Some(time),
        }
    }
}

#[derive(Debug, Default)]
pub struct TrainingTracker {
    seq: u32,
    open: Option<OpenAttempt>,
    last_blue_score: Option<i32>,
    pending_retry: bool,
}

impl TrainingTracker {
    pub fn round_started(&mut self, at_ms: i64) -> Option<Attempt> {
        let finished = self.close(at_ms);
        self.open = Some(OpenAttempt::new(
            at_ms,
            std::mem::take(&mut self.pending_retry),
        ));
        finished
    }

    /// The next shot's preview started: the current attempt is over.
    pub fn countdown_began(&mut self, at_ms: i64) -> Option<Attempt> {
        self.close(at_ms)
    }

    pub fn update(&mut self, game: &GameState) {
        let blue = game.team_score(0);
        let dropped = self.last_blue_score.is_some_and(|prev| blue < prev);
        self.last_blue_score = Some(blue);
        match self.open.as_mut() {
            Some(open) => {
                open.retry_after_make |= dropped;
                open.observe_time(game.time_seconds);
            }
            None => self.pending_retry |= dropped,
        }
    }

    pub fn goal(&mut self, at_ms: i64, speed: f64) {
        if let Some(open) = self.open.as_mut() {
            if open.goal.is_none() {
                open.goal = Some((at_ms, speed));
            }
        }
    }

    pub fn finish(&mut self, at_ms: i64) -> Option<Attempt> {
        self.close(at_ms)
    }

    fn close(&mut self, at_ms: i64) -> Option<Attempt> {
        let open = self.open.take()?;
        let (outcome, end_ms, goal_speed) = match open.goal {
            Some((goal_at, speed)) => (AttemptOutcome::Make, goal_at, Some(speed)),
            None => {
                if !open.ticked || at_ms - open.started_at_ms < MIN_ATTEMPT_MS {
                    self.pending_retry |= open.retry_after_make;
                    return None;
                }
                (AttemptOutcome::Miss, at_ms, None)
            }
        };
        self.seq += 1;
        Some(Attempt {
            seq: self.seq,
            started_at_ms: open.started_at_ms,
            duration_ms: end_ms - open.started_at_ms,
            outcome,
            retry_after_make: open.retry_after_make,
            goal_speed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::events::{GameState, TeamState};
    use crate::facts::AttemptOutcome::*;

    fn game(time: f64, blue: i32) -> GameState {
        GameState {
            time_seconds: time,
            teams: vec![TeamState {
                team_num: 0,
                score: blue,
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn tick_down(t: &mut TrainingTracker, from: i32, to: i32, blue: i32) {
        for s in (to..=from).rev() {
            t.update(&game(f64::from(s), blue));
        }
    }

    #[test]
    fn goal_is_a_make_with_its_speed() {
        let mut t = TrainingTracker::default();
        assert!(t.round_started(0).is_none());
        tick_down(&mut t, 10, 8, 0);
        t.goal(3_000, 80.5);
        let a = t.round_started(4_000).unwrap();
        assert_eq!(
            (a.seq, a.outcome, a.duration_ms, a.goal_speed),
            (1, Make, 3_000, Some(80.5))
        );
    }

    #[test]
    fn early_reset_without_goal_is_a_miss() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        tick_down(&mut t, 10, 7, 0);
        let a = t.round_started(3_500).unwrap();
        assert_eq!(a.outcome, Miss);
    }

    #[test]
    fn timer_reaching_zero_is_a_miss_even_after_the_preview_resets_it() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        tick_down(&mut t, 10, 0, 0);
        t.update(&game(10.0, 0)); // next shot's preview, player still waiting
        assert_eq!(t.round_started(25_000).unwrap().outcome, Miss);
    }

    #[test]
    fn countdown_closes_the_attempt_at_the_preview() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        tick_down(&mut t, 10, 5, 0);
        let a = t.countdown_began(5_500).unwrap();
        assert_eq!((a.outcome, a.duration_ms), (Miss, 5_500));
        t.update(&game(10.0, 0));
        assert!(t.round_started(20_000).is_none(), "already closed");
        assert!(
            t.countdown_began(20_100).is_none(),
            "skipping previews opens nothing"
        );
    }

    #[test]
    fn reset_artefacts_are_discarded() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        t.update(&game(10.0, 0));
        assert!(t.round_started(100).is_none(), "under 1 s");
        t.update(&game(10.0, 0));
        assert!(
            t.finish(30_000).is_none(),
            "timer never ticked (paused, then left)"
        );
    }

    #[test]
    fn score_drop_marks_retry_and_survives_a_discarded_attempt() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        tick_down(&mut t, 10, 8, 0);
        t.goal(2_000, 70.0);
        t.update(&game(8.0, 1));
        assert_eq!(t.round_started(3_000).unwrap().outcome, Make);
        // Retry button: an instant extra RoundStarted while the score drops back.
        t.update(&game(10.0, 0));
        assert!(t.round_started(3_100).is_none());
        tick_down(&mut t, 10, 6, 0);
        let a = t.finish(8_000).unwrap();
        assert_eq!((a.seq, a.outcome, a.retry_after_make), (2, Miss, true));
    }

    #[test]
    fn finish_closes_the_open_attempt() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        tick_down(&mut t, 10, 9, 0);
        t.goal(1_500, 60.0);
        assert_eq!(t.finish(9_000).unwrap().outcome, Make);
        assert!(t.finish(9_500).is_none());
    }

    #[test]
    fn stale_low_sample_from_the_previous_shot_does_not_hide_an_attempt() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        t.update(&game(3.0, 0)); // previous shot's timer, delivered after RoundStarted
        tick_down(&mut t, 10, 7, 0);
        assert_eq!(t.round_started(3_500).unwrap().outcome, Miss);
    }

    #[test]
    fn a_reset_shot_and_a_timed_out_shot_are_both_misses() {
        let mut t = TrainingTracker::default();
        t.round_started(0);
        t.update(&game(0.0, 0)); // stale zero from a timed-out shot
        tick_down(&mut t, 10, 6, 0);
        assert_eq!(t.round_started(4_500).unwrap().outcome, Miss, "reset");
        tick_down(&mut t, 10, 0, 0);
        assert_eq!(t.round_started(16_000).unwrap().outcome, Miss, "timeout");
    }
}
